-- Explicit upgrade only. Existing roots/projects/scans are retained unchanged.
-- The caller backs up then executes this file inside BEGIN IMMEDIATE.
CREATE TABLE project_profiles (
  project_id TEXT PRIMARY KEY REFERENCES projects(id),
  economics_mode TEXT NOT NULL DEFAULT 'balanced' CHECK(economics_mode IN ('maximum_intelligence','balanced','stretch','economy')),
  focus_json TEXT CHECK(focus_json IS NULL OR (json_valid(focus_json) AND length(focus_json)<=16384)),
  updated_at INTEGER NOT NULL CHECK(updated_at>=0)
) STRICT;
INSERT INTO project_profiles(project_id,updated_at) SELECT id,0 FROM projects;
CREATE TRIGGER project_profile_create AFTER INSERT ON projects BEGIN
  INSERT INTO project_profiles(project_id,updated_at) VALUES(NEW.id,0);
END;
CREATE TABLE environments (
  project_id TEXT NOT NULL REFERENCES projects(id),
  name TEXT NOT NULL CHECK(length(name) BETWEEN 1 AND 64),
  PRIMARY KEY(project_id,name)
) STRICT;
CREATE TABLE agent_installations (
  id TEXT PRIMARY KEY, product TEXT NOT NULL, version TEXT,
  adapter TEXT NOT NULL, payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=16384),
  observed_at INTEGER NOT NULL, coverage TEXT NOT NULL CHECK(coverage IN ('complete','partial','denied','unsupported','timed_out'))
) STRICT;
CREATE TABLE sessions (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id), agent TEXT NOT NULL,
  external_id TEXT NOT NULL CHECK(length(external_id) BETWEEN 1 AND 256),
  workspace_id TEXT, last_observed INTEGER NOT NULL CHECK(last_observed>=0), ended_at INTEGER,
  payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=16384),
  UNIQUE(id,project_id), UNIQUE(project_id,agent,external_id),
  FOREIGN KEY(project_id,workspace_id) REFERENCES workspace_registrations(project_id,id),
  CHECK(ended_at IS NULL OR ended_at>=last_observed)
) STRICT;
CREATE TABLE work_items (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id),
  state TEXT NOT NULL CHECK(state IN ('planned','active','paused','blocked','review','done','cancelled')),
  version INTEGER NOT NULL CHECK(version>=0), workspace_id TEXT,
  payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=32768),
  updated_at INTEGER NOT NULL CHECK(updated_at>=0), UNIQUE(id,project_id),
  FOREIGN KEY(project_id,workspace_id) REFERENCES workspace_registrations(project_id,id),
  CHECK(json_extract(payload,'$.state') IS state),CHECK(json_extract(payload,'$.version') IS version)
) STRICT;
CREATE TRIGGER work_state_guard BEFORE UPDATE OF state ON work_items
WHEN OLD.state != NEW.state AND NOT (
 (OLD.state='planned' AND NEW.state IN ('active','blocked','cancelled')) OR
 (OLD.state='active' AND NEW.state IN ('paused','blocked','review','cancelled')) OR
 (OLD.state IN ('paused','blocked') AND NEW.state IN ('active','cancelled')) OR
 (OLD.state='review' AND NEW.state IN ('active','blocked','done','cancelled'))
) BEGIN SELECT RAISE(ABORT,'INVALID_WORK_TRANSITION'); END;
CREATE TABLE assignments (
  id TEXT PRIMARY KEY, work_id TEXT NOT NULL, project_id TEXT NOT NULL, session_id TEXT NOT NULL,
  role TEXT NOT NULL CHECK(role IN ('primary','reviewer','observer')),
  lease_until INTEGER NOT NULL, created_at INTEGER NOT NULL, released_at INTEGER,
  FOREIGN KEY(work_id,project_id) REFERENCES work_items(id,project_id),
  FOREIGN KEY(session_id,project_id) REFERENCES sessions(id,project_id),
  CHECK(lease_until>=created_at),CHECK(released_at IS NULL OR released_at>=created_at)
) STRICT;
CREATE UNIQUE INDEX one_primary ON assignments(work_id) WHERE role='primary' AND released_at IS NULL;
CREATE UNIQUE INDEX one_live_assignment ON assignments(work_id,session_id,role) WHERE released_at IS NULL;
CREATE TABLE workspace_registrations (
 project_id TEXT NOT NULL REFERENCES projects(id),id TEXT NOT NULL,path TEXT NOT NULL,directory_identity TEXT NOT NULL,
 PRIMARY KEY(project_id,id),UNIQUE(project_id,path)
) STRICT;
CREATE TABLE workspace_observations (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id), workspace_id TEXT NOT NULL,
  observed_at INTEGER NOT NULL, source_kind TEXT NOT NULL CHECK(source_kind IN ('observed','agent_reported')),
  payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=32768),
  FOREIGN KEY(project_id,workspace_id) REFERENCES workspace_registrations(project_id,id)
) STRICT;
CREATE INDEX workspace_latest ON workspace_observations(project_id,workspace_id,observed_at DESC);
CREATE TABLE decisions (
  id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),topic TEXT NOT NULL,scope TEXT NOT NULL,
  predecessor TEXT,effective_at INTEGER NOT NULL CHECK(effective_at>=0),recorded_at INTEGER NOT NULL CHECK(recorded_at>=0),
  payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=32768),
  UNIQUE(project_id,topic,scope,id),CHECK(id IS NOT predecessor),
  FOREIGN KEY(project_id,topic,scope,predecessor) REFERENCES decisions(project_id,topic,scope,id)
) STRICT;
CREATE TABLE decision_acceptances (
  decision_id TEXT PRIMARY KEY,project_id TEXT NOT NULL,topic TEXT NOT NULL,scope TEXT NOT NULL,
  predecessor TEXT,accepted_at INTEGER NOT NULL CHECK(accepted_at>=0),approval_ref TEXT NOT NULL,
  FOREIGN KEY(project_id,topic,scope,decision_id) REFERENCES decisions(project_id,topic,scope,id),
  FOREIGN KEY(predecessor) REFERENCES decision_acceptances(decision_id)
) STRICT;
CREATE UNIQUE INDEX one_decision_root ON decision_acceptances(project_id,topic,scope) WHERE predecessor IS NULL;
CREATE UNIQUE INDEX one_decision_successor ON decision_acceptances(predecessor) WHERE predecessor IS NOT NULL;
CREATE TRIGGER decision_accept_guard BEFORE INSERT ON decision_acceptances BEGIN
 SELECT CASE WHEN NOT EXISTS(SELECT 1 FROM decisions d WHERE d.id=NEW.decision_id AND d.predecessor IS NEW.predecessor AND d.recorded_at<=NEW.accepted_at) THEN RAISE(ABORT,'DECISION_ACCEPTANCE_MISMATCH') END;
 SELECT CASE WHEN NEW.predecessor IS NOT NULL AND EXISTS(SELECT 1 FROM decision_acceptances parent WHERE parent.decision_id=NEW.predecessor AND parent.accepted_at>NEW.accepted_at) THEN RAISE(ABORT,'SUPERSESSION_KNOWLEDGE_TIME') END;
 SELECT CASE WHEN NEW.predecessor IS NOT NULL AND EXISTS(
 SELECT 1 FROM decisions child JOIN decisions parent ON parent.id=NEW.predecessor WHERE child.id=NEW.decision_id AND child.effective_at<parent.effective_at
 ) THEN RAISE(ABORT,'SUPERSESSION_EFFECTIVE_TIME') END;
END;
CREATE TABLE decision_dispositions (
 decision_id TEXT PRIMARY KEY REFERENCES decisions(id), disposition TEXT NOT NULL CHECK(disposition IN ('rejected','withdrawn')),
 recorded_at INTEGER NOT NULL,approval_ref TEXT NOT NULL
) STRICT;
CREATE TRIGGER reject_accepted BEFORE INSERT ON decision_dispositions WHEN EXISTS(SELECT 1 FROM decision_acceptances WHERE decision_id=NEW.decision_id) BEGIN SELECT RAISE(ABORT,'ACCEPTED_USE_SUPERSESSION'); END;
CREATE TRIGGER accept_rejected BEFORE INSERT ON decision_acceptances WHEN EXISTS(SELECT 1 FROM decision_dispositions WHERE decision_id=NEW.decision_id) BEGIN SELECT RAISE(ABORT,'DISPOSED_PROPOSAL'); END;
CREATE TABLE resources (
 id TEXT PRIMARY KEY,project_id TEXT NOT NULL,environment TEXT NOT NULL,kind TEXT NOT NULL,
 payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=8192),recorded_at INTEGER NOT NULL,
 UNIQUE(id,project_id),FOREIGN KEY(project_id,environment) REFERENCES environments(project_id,name),
 CHECK(kind IN ('service','endpoint','deployment','database','runbook','secret_reference','reference'))
) STRICT;
CREATE TABLE resource_verifications (
 id TEXT PRIMARY KEY,resource_id TEXT NOT NULL REFERENCES resources(id),claim TEXT NOT NULL,
 observed_at INTEGER NOT NULL,payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=8192),
 CHECK(claim IN ('user_confirmed','dns_resolved','http_reachable','provider_identity_verified'))
) STRICT;
CREATE TABLE checkpoints (
 id TEXT PRIMARY KEY,work_id TEXT NOT NULL,project_id TEXT NOT NULL,session_id TEXT,workspace_id TEXT NOT NULL,
 head TEXT NOT NULL,stamp_digest TEXT NOT NULL,recorded_at INTEGER NOT NULL,
 payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=65536),
 FOREIGN KEY(work_id,project_id) REFERENCES work_items(id,project_id),FOREIGN KEY(session_id,project_id) REFERENCES sessions(id,project_id),
 UNIQUE(id,project_id)
) STRICT;
CREATE TABLE handoffs (
 id TEXT PRIMARY KEY,checkpoint_id TEXT NOT NULL,project_id TEXT NOT NULL,target_agent TEXT NOT NULL,
 packet_digest TEXT NOT NULL,approval_ref TEXT,created_at INTEGER NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('prepared','delivered_reported','failed_reported','stale')),
 FOREIGN KEY(checkpoint_id,project_id) REFERENCES checkpoints(id,project_id)
) STRICT;
CREATE TABLE quota_samples (
 id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),provider TEXT NOT NULL,account_alias TEXT NOT NULL,bucket TEXT NOT NULL,
 meter TEXT NOT NULL CHECK(meter IN ('context','subscription','dollars')),unit TEXT NOT NULL,window_id TEXT NOT NULL,
 used REAL NOT NULL CHECK(used>=0 AND used<=1e15),capacity REAL CHECK(capacity>0 AND capacity<=1e15),reset_at INTEGER,observed_at INTEGER NOT NULL,
 payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=8192),
 UNIQUE(project_id,provider,account_alias,bucket,meter,unit,window_id,observed_at)
) STRICT;
CREATE INDEX quota_time ON quota_samples(project_id,provider,account_alias,bucket,observed_at DESC);
CREATE TABLE capability_candidates (
 id TEXT PRIMARY KEY,source_digest TEXT NOT NULL UNIQUE,canonical_url TEXT NOT NULL,
 review_status TEXT NOT NULL CHECK(review_status IN ('unverified','reviewed','rejected')),
 payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=16384),recorded_at INTEGER NOT NULL
) STRICT;
CREATE TABLE capability_feedback (
 id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),resource_id TEXT NOT NULL,need TEXT NOT NULL,
 disposition TEXT NOT NULL CHECK(disposition IN ('saved','dismissed','snoozed','evaluated','adopted')),
 until_time INTEGER,outcome TEXT NOT NULL CHECK(outcome IN ('not_recorded','useful','not_useful','inconclusive')),
 evidence_ref TEXT,revision TEXT,recorded_at INTEGER NOT NULL,
 CHECK(disposition!='snoozed' OR (until_time IS NOT NULL AND until_time>recorded_at)),
 CHECK(outcome='not_recorded' OR (disposition='evaluated' AND evidence_ref IS NOT NULL AND revision IS NOT NULL))
) STRICT;
CREATE TABLE plans (
 id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),digest TEXT NOT NULL UNIQUE,
 created_at INTEGER NOT NULL,expires_at INTEGER NOT NULL,policy_version TEXT NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('proposed','approved','blocked','expired','applied','failed','indeterminate')),
 payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=32768),CHECK(expires_at>created_at AND expires_at<=created_at+900)
) STRICT;
CREATE TABLE approval_events (
 id TEXT PRIMARY KEY,plan_id TEXT NOT NULL REFERENCES plans(id),digest TEXT NOT NULL,recorded_at INTEGER NOT NULL,actor TEXT NOT NULL
) STRICT;
CREATE TABLE audit_events (
 seq INTEGER PRIMARY KEY AUTOINCREMENT,event_id TEXT NOT NULL UNIQUE,project_id TEXT REFERENCES projects(id),kind TEXT NOT NULL,
 recorded_at INTEGER NOT NULL,source_kind TEXT NOT NULL CHECK(source_kind IN ('observed','agent_reported','inferred','user_approved','provider_verified','unknown')),
 payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=16384)
) STRICT;
CREATE INDEX audit_project ON audit_events(project_id,seq DESC);
CREATE TRIGGER audit_no_update BEFORE UPDATE ON audit_events BEGIN SELECT RAISE(ABORT,'IMMUTABLE_AUDIT'); END;
CREATE TRIGGER audit_no_delete BEFORE DELETE ON audit_events BEGIN SELECT RAISE(ABORT,'IMMUTABLE_AUDIT'); END;
CREATE TRIGGER decisions_no_update BEFORE UPDATE ON decisions BEGIN SELECT RAISE(ABORT,'IMMUTABLE_DECISION'); END;
CREATE TRIGGER decisions_no_delete BEFORE DELETE ON decisions BEGIN SELECT RAISE(ABORT,'IMMUTABLE_DECISION'); END;
CREATE TRIGGER acceptances_no_update BEFORE UPDATE ON decision_acceptances BEGIN SELECT RAISE(ABORT,'IMMUTABLE_ACCEPTANCE'); END;
CREATE TRIGGER acceptances_no_delete BEFORE DELETE ON decision_acceptances BEGIN SELECT RAISE(ABORT,'IMMUTABLE_ACCEPTANCE'); END;
CREATE TRIGGER checkpoints_no_update BEFORE UPDATE ON checkpoints BEGIN SELECT RAISE(ABORT,'IMMUTABLE_CHECKPOINT'); END;
CREATE TRIGGER checkpoints_no_delete BEFORE DELETE ON checkpoints BEGIN SELECT RAISE(ABORT,'IMMUTABLE_CHECKPOINT'); END;
CREATE TRIGGER approved_plan_payload_immutable BEFORE UPDATE OF payload,digest,expires_at,policy_version ON plans WHEN OLD.state!='proposed' BEGIN SELECT RAISE(ABORT,'APPROVED_PLAN_IMMUTABLE'); END;
PRAGMA user_version=2;
