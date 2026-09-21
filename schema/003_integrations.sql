-- Invoked only by explicit backed-up upgrade. No user application state is changed.
CREATE TABLE integration_profiles (
 id TEXT PRIMARY KEY,project_id TEXT NOT NULL,environment TEXT NOT NULL,adapter TEXT NOT NULL,
 digest TEXT NOT NULL CHECK(length(digest)=64),payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=32768),
 created_at INTEGER NOT NULL,FOREIGN KEY(project_id,environment) REFERENCES environments(project_id,name)
) STRICT;
CREATE TABLE approved_tasks (
 id TEXT PRIMARY KEY,project_id TEXT NOT NULL,environment TEXT NOT NULL,digest TEXT NOT NULL CHECK(length(digest)=64),
 payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=65536),created_at INTEGER NOT NULL,
 FOREIGN KEY(project_id,environment) REFERENCES environments(project_id,name)
) STRICT;
CREATE TABLE effect_plans (
 id TEXT PRIMARY KEY,project_id TEXT NOT NULL,environment TEXT NOT NULL,policy TEXT NOT NULL,
 digest TEXT NOT NULL CHECK(length(digest)=64),created_at INTEGER NOT NULL,expires_at INTEGER NOT NULL,
 payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=131072),
 FOREIGN KEY(project_id,environment) REFERENCES environments(project_id,name),
 CHECK(expires_at>created_at AND expires_at-created_at<=900)
) STRICT;
CREATE TABLE effect_runs (
 id TEXT PRIMARY KEY,plan_id TEXT NOT NULL UNIQUE REFERENCES effect_plans(id),
 state TEXT NOT NULL CHECK(state IN ('running','succeeded','failed','indeterminate','cancelled')),
 started_at INTEGER NOT NULL,finished_at INTEGER,error_code TEXT,
 receipt TEXT CHECK(receipt IS NULL OR (json_valid(receipt) AND length(receipt)<=262144))
) STRICT;
CREATE TABLE effect_steps (
 run_id TEXT NOT NULL REFERENCES effect_runs(id),seq INTEGER NOT NULL,
 name TEXT NOT NULL,status TEXT NOT NULL CHECK(status IN ('intent','completed','failed','indeterminate')),
 observed_at INTEGER NOT NULL,payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=32768),
 PRIMARY KEY(run_id,seq)
) STRICT;
CREATE TABLE lifecycle_events (
 source_id TEXT NOT NULL,project_id TEXT NOT NULL REFERENCES projects(id),digest TEXT NOT NULL CHECK(length(digest)=64),
 payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=8192),received_at INTEGER NOT NULL,
 PRIMARY KEY(project_id,source_id)
) STRICT;
CREATE TABLE observation_caches (
 project_id TEXT NOT NULL REFERENCES projects(id),category TEXT NOT NULL,
 observed_at INTEGER NOT NULL,payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=262144),
 PRIMARY KEY(project_id,category)
) STRICT;
CREATE TABLE continuation_runs (
 effect_run_id TEXT PRIMARY KEY REFERENCES effect_runs(id),work_id TEXT NOT NULL REFERENCES work_items(id),
 checkpoint_id TEXT NOT NULL REFERENCES checkpoints(id),target_session_id TEXT NOT NULL REFERENCES sessions(id),
 state TEXT NOT NULL CHECK(state IN ('starting','running','completed','failed','uncertain')),created_at INTEGER NOT NULL
) STRICT;
CREATE TRIGGER effect_plan_immutable BEFORE UPDATE ON effect_plans BEGIN SELECT RAISE(ABORT,'EFFECT_PLAN_IMMUTABLE');END;
CREATE TRIGGER effect_plan_no_delete BEFORE DELETE ON effect_plans BEGIN SELECT RAISE(ABORT,'EFFECT_PLAN_PRESERVED');END;
CREATE TRIGGER effect_step_no_update BEFORE UPDATE ON effect_steps BEGIN SELECT RAISE(ABORT,'STEP_APPEND_ONLY');END;
CREATE TRIGGER effect_step_no_delete BEFORE DELETE ON effect_steps BEGIN SELECT RAISE(ABORT,'STEP_PRESERVED');END;
CREATE TRIGGER effect_run_terminal BEFORE UPDATE ON effect_runs
 WHEN OLD.state!='running' BEGIN SELECT RAISE(ABORT,'TERMINAL_RUN_IMMUTABLE');END;
CREATE TRIGGER integration_profile_immutable BEFORE UPDATE ON integration_profiles BEGIN SELECT RAISE(ABORT,'PROFILE_IS_VERSIONED');END;
CREATE TRIGGER task_immutable BEFORE UPDATE ON approved_tasks BEGIN SELECT RAISE(ABORT,'TASK_IS_VERSIONED');END;
CREATE TRIGGER effect_run_no_delete BEFORE DELETE ON effect_runs BEGIN SELECT RAISE(ABORT,'RUN_AUDIT_PRESERVED');END;
PRAGMA user_version=3;
