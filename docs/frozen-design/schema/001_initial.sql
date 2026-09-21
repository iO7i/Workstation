-- Workstation FS-1.0: reference initial metadata schema, not the complete application.
-- Run on a new local database. Production must use a tested, patched SQLite build.
-- No secret value column exists. JSON/text still require domain validation and redaction.
-- WAL and busy-timeout selection happen in the connection layer, not against vendor DBs.
PRAGMA foreign_keys = ON;
BEGIN;
CREATE TABLE schema_migrations (
  version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL, application_version TEXT NOT NULL
);
CREATE TABLE projects (
  id TEXT PRIMARY KEY, slug TEXT NOT NULL UNIQUE, display_name TEXT NOT NULL,
  created_at TEXT NOT NULL, archived_at TEXT
);
CREATE TABLE repositories (
  id TEXT PRIMARY KEY, common_dir TEXT NOT NULL, volume_id TEXT, file_id TEXT,
  trust_state TEXT NOT NULL CHECK(trust_state IN ('untrusted','approved','revoked')),
  observed_at TEXT NOT NULL, UNIQUE(volume_id,file_id)
);
CREATE TABLE project_repositories (
  project_id TEXT NOT NULL REFERENCES projects(id), repository_id TEXT NOT NULL REFERENCES repositories(id),
  role TEXT NOT NULL, PRIMARY KEY(project_id,repository_id)
);
CREATE TABLE environments (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id), name TEXT NOT NULL,
  criticality TEXT NOT NULL CHECK(criticality IN ('local','development','staging','production','other')),
  UNIQUE(project_id,name), UNIQUE(project_id,id)
);
CREATE TABLE installations (
  id TEXT PRIMARY KEY, vendor TEXT NOT NULL, product_variant TEXT NOT NULL, version TEXT,
  executable_path TEXT, binary_digest TEXT, adapter_version TEXT NOT NULL,
  capabilities_json TEXT NOT NULL CHECK(json_valid(capabilities_json)), observed_at TEXT NOT NULL
);
CREATE TABLE objects (
  id TEXT PRIMARY KEY, kind TEXT NOT NULL, project_id TEXT REFERENCES projects(id),
  external_id TEXT, identity_json TEXT NOT NULL CHECK(json_valid(identity_json)),
  first_seen_at TEXT NOT NULL, last_seen_at TEXT NOT NULL
);
CREATE INDEX objects_project_kind ON objects(project_id,kind);
CREATE TABLE scan_runs (
  id TEXT PRIMARY KEY, host_id TEXT NOT NULL, boot_id TEXT, mode TEXT NOT NULL,
  started_at TEXT NOT NULL, completed_at TEXT,
  status TEXT NOT NULL CHECK(status IN ('running','complete','partial','failed','cancelled')),
  policy_revision TEXT NOT NULL, limits_json TEXT NOT NULL CHECK(json_valid(limits_json))
);
CREATE TABLE sources (
  id TEXT PRIMARY KEY, project_id TEXT REFERENCES projects(id), kind TEXT NOT NULL,
  locator TEXT NOT NULL, content_digest TEXT, consent_id TEXT,
  availability TEXT NOT NULL CHECK(availability IN ('available','partial','missing','denied','unknown')),
  recorded_at TEXT NOT NULL
);
CREATE TABLE observations (
  id TEXT PRIMARY KEY, scan_id TEXT NOT NULL REFERENCES scan_runs(id),
  object_id TEXT REFERENCES objects(id), source_id TEXT REFERENCES sources(id),
  collector_id TEXT NOT NULL, collector_version TEXT NOT NULL,
  observed_at TEXT NOT NULL,
  coverage TEXT NOT NULL CHECK(coverage IN ('complete','partial','denied','unsupported','timed_out')),
  evidence_type TEXT NOT NULL,
  values_json TEXT NOT NULL CHECK(json_valid(values_json)),
  limitations_json TEXT NOT NULL CHECK(json_valid(limitations_json))
);
CREATE INDEX observations_object_time ON observations(object_id,observed_at);
CREATE TABLE relationships (
  id TEXT PRIMARY KEY, from_object_id TEXT NOT NULL REFERENCES objects(id),
  to_object_id TEXT NOT NULL REFERENCES objects(id), relation_type TEXT NOT NULL,
  confidence TEXT NOT NULL CHECK(confidence IN ('confirmed','supported','suspected','unknown')),
  observation_id TEXT NOT NULL REFERENCES observations(id),
  valid_from TEXT NOT NULL, valid_to TEXT,
  CHECK(from_object_id <> to_object_id), CHECK(valid_to IS NULL OR valid_to >= valid_from)
);
CREATE INDEX relationships_from_type ON relationships(from_object_id,relation_type);
CREATE TABLE findings (
  id TEXT PRIMARY KEY, scan_id TEXT NOT NULL REFERENCES scan_runs(id),
  project_id TEXT REFERENCES projects(id), fingerprint_id TEXT NOT NULL,
  fingerprint_version TEXT NOT NULL, target_id TEXT REFERENCES objects(id),
  severity TEXT NOT NULL CHECK(severity IN ('info','warning','critical')),
  confidence TEXT NOT NULL CHECK(confidence IN ('confirmed','supported','suspected','unknown')),
  state TEXT NOT NULL CHECK(state IN ('open','protected','blocked','resolved','unknown','dismissed')),
  summary TEXT NOT NULL, first_observed_at TEXT NOT NULL, last_observed_at TEXT NOT NULL
);
CREATE TABLE finding_evidence (
  finding_id TEXT NOT NULL REFERENCES findings(id), observation_id TEXT NOT NULL REFERENCES observations(id),
  role TEXT NOT NULL CHECK(role IN ('supports','contradicts','limitation')),
  PRIMARY KEY(finding_id,observation_id,role)
);
CREATE TABLE workspaces (
  id TEXT PRIMARY KEY REFERENCES objects(id), repository_id TEXT NOT NULL REFERENCES repositories(id),
  path TEXT NOT NULL, head_oid TEXT, branch_ref TEXT,
  git_state_json TEXT NOT NULL CHECK(json_valid(git_state_json)),
  disposition TEXT NOT NULL CHECK(disposition IN ('active','protected','candidate','blocked','retired','unknown')),
  observed_at TEXT NOT NULL
);
CREATE TABLE sessions (
  id TEXT PRIMARY KEY REFERENCES objects(id), installation_id TEXT REFERENCES installations(id),
  vendor_session_id TEXT, workspace_id TEXT REFERENCES workspaces(id),
  activity_state TEXT NOT NULL CHECK(activity_state IN ('active','idle','ended','unknown')),
  first_seen_at TEXT NOT NULL, last_seen_at TEXT NOT NULL
);
CREATE TABLE protections (
  id TEXT PRIMARY KEY, object_id TEXT NOT NULL REFERENCES objects(id),
  kind TEXT NOT NULL CHECK(kind IN ('manual','lease','active_dependency','unknown_ownership','rollback')),
  actor TEXT NOT NULL, reason TEXT NOT NULL, created_at TEXT NOT NULL,
  expires_at TEXT, revoked_at TEXT
);
CREATE TABLE events (
  id TEXT PRIMARY KEY, project_id TEXT REFERENCES projects(id), kind TEXT NOT NULL,
  summary TEXT NOT NULL, effective_at TEXT NOT NULL, recorded_at TEXT NOT NULL,
  source_id TEXT REFERENCES sources(id), payload_json TEXT NOT NULL CHECK(json_valid(payload_json))
);
CREATE INDEX events_project_time ON events(project_id,effective_at,recorded_at);
CREATE TABLE decisions (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id), topic_key TEXT NOT NULL,
  scope_key TEXT NOT NULL, scope_json TEXT NOT NULL CHECK(json_valid(scope_json)),
  statement TEXT NOT NULL, rationale TEXT NOT NULL,
  alternatives_json TEXT NOT NULL CHECK(json_valid(alternatives_json)),
  consequences_json TEXT NOT NULL CHECK(json_valid(consequences_json)),
  author TEXT NOT NULL, effective_at TEXT NOT NULL, recorded_at TEXT NOT NULL,
  predecessor_id TEXT REFERENCES decisions(id),
  CHECK(predecessor_id IS NULL OR predecessor_id <> id),
  UNIQUE(project_id,topic_key,scope_key,id)
);
-- Decision records are revisions. Changes create a new revision, not an UPDATE.
CREATE TRIGGER decision_no_update BEFORE UPDATE ON decisions BEGIN
  SELECT RAISE(ABORT,'decision revisions are immutable');
END;
CREATE TRIGGER decision_no_delete BEFORE DELETE ON decisions BEGIN
  SELECT RAISE(ABORT,'decision revisions require an explicit retention migration');
END;
CREATE TABLE decision_sources (
  decision_id TEXT NOT NULL REFERENCES decisions(id), source_id TEXT NOT NULL REFERENCES sources(id),
  relation TEXT NOT NULL, PRIMARY KEY(decision_id,source_id)
);
CREATE TABLE decision_transitions (
  id TEXT PRIMARY KEY, decision_id TEXT NOT NULL REFERENCES decisions(id),
  action TEXT NOT NULL CHECK(action IN ('proposed','accepted','rejected','withdrawn','superseded')),
  actor TEXT NOT NULL, effective_at TEXT NOT NULL, recorded_at TEXT NOT NULL,
  replacement_id TEXT REFERENCES decisions(id), approval_id TEXT,
  CHECK(replacement_id IS NULL OR replacement_id <> decision_id),
  CHECK(action <> 'superseded' OR replacement_id IS NOT NULL),
  CHECK(action <> 'accepted' OR approval_id IS NOT NULL)
);
CREATE INDEX decision_transitions_asof ON decision_transitions(decision_id,recorded_at,effective_at);
CREATE TRIGGER decision_transition_no_update BEFORE UPDATE ON decision_transitions BEGIN
  SELECT RAISE(ABORT,'decision transitions are append-only');
END;
CREATE TRIGGER decision_transition_no_delete BEFORE DELETE ON decision_transitions BEGIN
  SELECT RAISE(ABORT,'decision transitions are append-only');
END;
-- This is the accepted revision-chain tip, not necessarily the decision in force now.
-- Domain code calculates effective-time/as-known-time views from the transition ledger.
CREATE TABLE decision_heads (
  project_id TEXT NOT NULL, topic_key TEXT NOT NULL, scope_key TEXT NOT NULL, decision_id TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY(project_id,topic_key,scope_key),
  FOREIGN KEY(project_id,topic_key,scope_key,decision_id)
    REFERENCES decisions(project_id,topic_key,scope_key,id)
);
CREATE TABLE capability_connections (
  id TEXT PRIMARY KEY, capability_id TEXT NOT NULL, provider TEXT,
  status TEXT NOT NULL CHECK(status IN ('proposed','connected','reference_only','disabled','unsupported')),
  config_json TEXT NOT NULL CHECK(json_valid(config_json)),
  receipt_json TEXT CHECK(receipt_json IS NULL OR json_valid(receipt_json)), updated_at TEXT NOT NULL
);
CREATE TABLE resources (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id), environment_id TEXT NOT NULL,
  kind TEXT NOT NULL, label TEXT NOT NULL, provider TEXT, provider_object_id TEXT,
  endpoint TEXT, locator_json TEXT NOT NULL CHECK(json_valid(locator_json)),
  authority TEXT NOT NULL CHECK(authority IN ('proposed','human_accepted','provider_observed','conflict','revoked')),
  source_id TEXT REFERENCES sources(id), recorded_at TEXT NOT NULL,
  FOREIGN KEY(project_id,environment_id) REFERENCES environments(project_id,id),
  UNIQUE(project_id,environment_id,id)
);
CREATE TABLE resource_verifications (
  id TEXT PRIMARY KEY, resource_id TEXT NOT NULL REFERENCES resources(id), kind TEXT NOT NULL,
  result TEXT NOT NULL CHECK(result IN ('passed','failed','unknown','blocked')),
  observed_at TEXT NOT NULL, expires_at TEXT,
  details_json TEXT NOT NULL CHECK(json_valid(details_json))
);
CREATE TABLE secret_refs (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id), environment_id TEXT NOT NULL,
  provider TEXT NOT NULL CHECK(provider IN ('local_dpapi','doppler','onepassword','infisical','bitwarden')),
  connection_id TEXT REFERENCES capability_connections(id), locator_json TEXT NOT NULL CHECK(json_valid(locator_json)),
  metadata_json TEXT NOT NULL CHECK(json_valid(metadata_json)), expose_to_agent INTEGER NOT NULL DEFAULT 0 CHECK(expose_to_agent=0),
  FOREIGN KEY(project_id,environment_id) REFERENCES environments(project_id,id)
);
CREATE TABLE plans (
  id TEXT PRIMARY KEY, finding_id TEXT REFERENCES findings(id), schema_version TEXT NOT NULL,
  host_id TEXT NOT NULL, user_sid TEXT NOT NULL, policy_revision TEXT NOT NULL,
  digest TEXT NOT NULL UNIQUE, body_json TEXT NOT NULL CHECK(json_valid(body_json)),
  created_at TEXT NOT NULL, expires_at TEXT NOT NULL,
  CHECK(expires_at > created_at)
);
CREATE TRIGGER plan_no_update BEFORE UPDATE ON plans BEGIN
  SELECT RAISE(ABORT,'approved targets require a new plan');
END;
CREATE TABLE approvals (
  id TEXT PRIMARY KEY, subject_type TEXT NOT NULL CHECK(subject_type IN ('plan','decision','import','task','integration')),
  subject_id TEXT NOT NULL, subject_digest TEXT NOT NULL,
  actor_sid TEXT NOT NULL, approved_at TEXT NOT NULL, expires_at TEXT, revoked_at TEXT
);
CREATE TABLE action_runs (
  id TEXT PRIMARY KEY, plan_id TEXT NOT NULL REFERENCES plans(id), approval_id TEXT NOT NULL REFERENCES approvals(id),
  status TEXT NOT NULL CHECK(status IN ('approved','running','succeeded','failed','blocked','interrupted_needs_review','cancelled')),
  started_at TEXT NOT NULL, finished_at TEXT, result_json TEXT CHECK(result_json IS NULL OR json_valid(result_json))
);
CREATE TABLE action_steps (
  id TEXT PRIMARY KEY, run_id TEXT NOT NULL REFERENCES action_runs(id), ordinal INTEGER NOT NULL CHECK(ordinal>=0),
  idempotency_key TEXT NOT NULL UNIQUE, action_kind TEXT NOT NULL, target_id TEXT NOT NULL REFERENCES objects(id),
  status TEXT NOT NULL CHECK(status IN ('planned','running','succeeded','failed','blocked','unknown')),
  precondition_json TEXT NOT NULL CHECK(json_valid(precondition_json)),
  receipt_json TEXT CHECK(receipt_json IS NULL OR json_valid(receipt_json)),
  UNIQUE(run_id,ordinal)
);
CREATE TABLE approved_tasks (
  id TEXT PRIMARY KEY, project_id TEXT NOT NULL REFERENCES projects(id), environment_id TEXT NOT NULL,
  task_digest TEXT NOT NULL, spec_json TEXT NOT NULL CHECK(json_valid(spec_json)),
  approval_id TEXT NOT NULL REFERENCES approvals(id), enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
  FOREIGN KEY(project_id,environment_id) REFERENCES environments(project_id,id)
);
CREATE TABLE audit_events (
  id TEXT PRIMARY KEY, event_type TEXT NOT NULL, actor TEXT NOT NULL, recorded_at TEXT NOT NULL,
  plan_id TEXT REFERENCES plans(id), payload_json TEXT NOT NULL CHECK(json_valid(payload_json))
);
CREATE TRIGGER audit_no_update BEFORE UPDATE ON audit_events BEGIN
  SELECT RAISE(ABORT,'audit is append-only within normal application operations');
END;
CREATE TRIGGER audit_no_delete BEFORE DELETE ON audit_events BEGIN
  SELECT RAISE(ABORT,'audit retention needs explicit maintenance mode');
END;
-- Opt-in approved-text index. Never populate with complete session transcripts by default.
CREATE VIRTUAL TABLE approved_text_fts USING fts5(entity_id UNINDEXED,project_id UNINDEXED,title,body);
INSERT INTO schema_migrations VALUES(1,'2026-09-18T00:00:00Z','design-reference');
COMMIT;
