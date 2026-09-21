-- Explicit backed-up v3 -> v4 migration. No external application mutation.
CREATE TABLE host_epochs (
 singleton INTEGER PRIMARY KEY CHECK(singleton=1),epoch_id TEXT NOT NULL,
 last_uptime_ms INTEGER NOT NULL CHECK(last_uptime_ms>=0),observed_at INTEGER NOT NULL
) STRICT;
CREATE TABLE managed_process_bindings (
 session_id TEXT NOT NULL REFERENCES sessions(id),host_id TEXT NOT NULL,boot_id TEXT NOT NULL,
 pid INTEGER NOT NULL CHECK(pid>0),creation_time TEXT NOT NULL,executable_digest TEXT NOT NULL CHECK(length(executable_digest)=64),
 source_run TEXT NOT NULL REFERENCES effect_runs(id),bound_at INTEGER NOT NULL,
 PRIMARY KEY(session_id,host_id,boot_id,pid,creation_time)
) STRICT;
-- A configured metadata hook can correlate observed caller ancestry, but cannot prove
-- exclusive ownership. These bindings are always shared/protected and expire quickly.
CREATE TABLE hook_process_bindings (
 session_id TEXT NOT NULL REFERENCES sessions(id), integration_id TEXT NOT NULL REFERENCES integration_profiles(id),
 host_id TEXT NOT NULL,boot_id TEXT NOT NULL,pid INTEGER NOT NULL CHECK(pid>0),creation_time TEXT NOT NULL,
 source_event TEXT NOT NULL,bound_at INTEGER NOT NULL,expires_at INTEGER NOT NULL CHECK(expires_at>=bound_at),
 PRIMARY KEY(session_id,host_id,boot_id,pid,creation_time)
) STRICT;
CREATE TABLE workspace_protections (
 id TEXT PRIMARY KEY,project_id TEXT NOT NULL,workspace_id TEXT NOT NULL,reason TEXT NOT NULL,
 created_at INTEGER NOT NULL,released_at INTEGER,
 FOREIGN KEY(project_id,workspace_id) REFERENCES workspace_registrations(project_id,id)
) STRICT;
CREATE TABLE workspace_retirements (
 project_id TEXT NOT NULL,workspace_id TEXT NOT NULL,run_id TEXT NOT NULL REFERENCES effect_runs(id),
 head_ref TEXT NOT NULL,recorded_at INTEGER NOT NULL,receipt TEXT NOT NULL CHECK(json_valid(receipt)),
 PRIMARY KEY(project_id,workspace_id),FOREIGN KEY(project_id,workspace_id) REFERENCES workspace_registrations(project_id,id)
) STRICT;
CREATE TABLE secret_versions (
 resource_id TEXT NOT NULL REFERENCES resources(id),version_id TEXT NOT NULL,
 file_name TEXT NOT NULL,cipher_digest TEXT NOT NULL CHECK(length(cipher_digest)=64),
 predecessor TEXT,created_at INTEGER NOT NULL,source_run TEXT NOT NULL REFERENCES effect_runs(id),
 PRIMARY KEY(resource_id,version_id),UNIQUE(file_name),
 FOREIGN KEY(resource_id,predecessor) REFERENCES secret_versions(resource_id,version_id)
) STRICT;
CREATE TABLE secret_heads (
 resource_id TEXT PRIMARY KEY REFERENCES resources(id),version_id TEXT NOT NULL,
 state TEXT NOT NULL CHECK(state IN ('active','revoked')),generation INTEGER NOT NULL CHECK(generation>0),
 changed_at INTEGER NOT NULL,FOREIGN KEY(resource_id,version_id) REFERENCES secret_versions(resource_id,version_id)
) STRICT;
CREATE TABLE secret_transitions (
 id TEXT PRIMARY KEY,resource_id TEXT NOT NULL REFERENCES resources(id),version_id TEXT NOT NULL,
 action TEXT NOT NULL CHECK(action IN ('activated','superseded','revoked')),at INTEGER NOT NULL,source_run TEXT NOT NULL REFERENCES effect_runs(id)
) STRICT;
CREATE TRIGGER secret_version_immutable BEFORE UPDATE ON secret_versions BEGIN SELECT RAISE(ABORT,'SECRET_VERSION_IMMUTABLE');END;
CREATE TRIGGER secret_version_preserved BEFORE DELETE ON secret_versions BEGIN SELECT RAISE(ABORT,'SECRET_CIPHERTEXT_REFERENCE_PRESERVED');END;
CREATE TRIGGER secret_transition_immutable BEFORE UPDATE ON secret_transitions BEGIN SELECT RAISE(ABORT,'SECRET_TRANSITION_IMMUTABLE');END;
CREATE TRIGGER secret_transition_preserved BEFORE DELETE ON secret_transitions BEGIN SELECT RAISE(ABORT,'SECRET_TRANSITION_PRESERVED');END;
CREATE TABLE journal_archives (
 id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),file_name TEXT NOT NULL UNIQUE,
 digest TEXT NOT NULL CHECK(length(digest)=64),byte_count INTEGER NOT NULL CHECK(byte_count BETWEEN 1 AND 8388608),
 record_count INTEGER NOT NULL CHECK(record_count>0),created_at INTEGER NOT NULL
) STRICT;
CREATE TABLE journal_archive_entries (
 kind TEXT NOT NULL CHECK(kind IN ('plan','receipt','step')),record_key TEXT NOT NULL,
 archive_id TEXT NOT NULL REFERENCES journal_archives(id),content_digest TEXT NOT NULL CHECK(length(content_digest)=64),
 PRIMARY KEY(kind,record_key)
) STRICT;
CREATE TRIGGER archive_immutable BEFORE UPDATE ON journal_archives BEGIN SELECT RAISE(ABORT,'ARCHIVE_IMMUTABLE');END;
CREATE TRIGGER archive_preserved BEFORE DELETE ON journal_archives BEGIN SELECT RAISE(ABORT,'ARCHIVE_PRESERVED');END;
CREATE TRIGGER archive_entry_immutable BEFORE UPDATE ON journal_archive_entries BEGIN SELECT RAISE(ABORT,'ARCHIVE_ENTRY_IMMUTABLE');END;
CREATE TRIGGER archive_entry_preserved BEFORE DELETE ON journal_archive_entries BEGIN SELECT RAISE(ABORT,'ARCHIVE_ENTRY_PRESERVED');END;
-- Logical records remain immutable. The only permitted physical update replaces a payload
-- with a verified archive locator, while preserving all original identifiers and digests.
DROP TRIGGER effect_plan_immutable;
CREATE TRIGGER effect_plan_immutable BEFORE UPDATE ON effect_plans WHEN NOT (
 NEW.id IS OLD.id AND NEW.project_id IS OLD.project_id AND NEW.environment IS OLD.environment
 AND NEW.policy IS OLD.policy AND NEW.digest IS OLD.digest AND NEW.created_at IS OLD.created_at AND NEW.expires_at IS OLD.expires_at
 AND EXISTS(SELECT 1 FROM journal_archive_entries a WHERE a.kind='plan' AND a.record_key=OLD.id
 AND a.archive_id=json_extract(NEW.payload,'$.workstation_archive.id') AND a.content_digest=OLD.digest
 AND a.content_digest=json_extract(NEW.payload,'$.workstation_archive.sha256'))
) BEGIN SELECT RAISE(ABORT,'EFFECT_PLAN_IMMUTABLE');END;
DROP TRIGGER effect_step_no_update;
CREATE TRIGGER effect_step_no_update BEFORE UPDATE ON effect_steps WHEN NOT (
 NEW.run_id IS OLD.run_id AND NEW.seq IS OLD.seq AND NEW.name IS OLD.name AND NEW.status IS OLD.status AND NEW.observed_at IS OLD.observed_at
 AND EXISTS(SELECT 1 FROM journal_archive_entries a WHERE a.kind='step' AND a.record_key=OLD.run_id||':'||OLD.seq
 AND a.archive_id=json_extract(NEW.payload,'$.workstation_archive.id') AND a.content_digest=json_extract(NEW.payload,'$.workstation_archive.sha256'))
) BEGIN SELECT RAISE(ABORT,'STEP_APPEND_ONLY');END;
DROP TRIGGER effect_run_terminal;
CREATE TRIGGER effect_run_terminal BEFORE UPDATE ON effect_runs WHEN OLD.state!='running' AND NOT (
 NEW.id IS OLD.id AND NEW.plan_id IS OLD.plan_id AND NEW.state IS OLD.state AND NEW.started_at IS OLD.started_at
 AND NEW.finished_at IS OLD.finished_at AND NEW.error_code IS OLD.error_code
 AND EXISTS(SELECT 1 FROM journal_archive_entries a WHERE a.kind='receipt' AND a.record_key=OLD.id
 AND a.archive_id=json_extract(NEW.receipt,'$.workstation_archive.id') AND a.content_digest=json_extract(NEW.receipt,'$.workstation_archive.sha256'))
) BEGIN SELECT RAISE(ABORT,'TERMINAL_RUN_IMMUTABLE');END;
PRAGMA user_version=4;
