-- Durable evidence stays separate from immutable external effect receipts.
CREATE TABLE external_runs (
 id TEXT PRIMARY KEY, plan_id TEXT NOT NULL UNIQUE REFERENCES effect_plans(id),
 project_id TEXT NOT NULL, environment TEXT NOT NULL,
 revision INTEGER NOT NULL CHECK(revision>=0), seq INTEGER NOT NULL CHECK(seq>=0),
 cancel_requested INTEGER NOT NULL DEFAULT 0 CHECK(cancel_requested IN (0,1)),
 updated_at INTEGER NOT NULL, payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=32768),
 FOREIGN KEY(project_id,environment) REFERENCES environments(project_id,name)
) STRICT;
CREATE INDEX external_runs_scope ON external_runs(project_id,updated_at DESC);
CREATE TABLE external_run_events (
 run_id TEXT NOT NULL REFERENCES external_runs(id), seq INTEGER NOT NULL,
 observed_at INTEGER NOT NULL,payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=4096),
 PRIMARY KEY(run_id,seq)
) STRICT;
CREATE TRIGGER durable_event_no_update BEFORE UPDATE ON external_run_events BEGIN SELECT RAISE(ABORT,'run events append only'); END;
CREATE TRIGGER durable_event_no_delete BEFORE DELETE ON external_run_events BEGIN SELECT RAISE(ABORT,'run events append only'); END;
CREATE TABLE external_run_baselines (
 run_id TEXT PRIMARY KEY REFERENCES external_runs(id),digest TEXT NOT NULL,
 payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=1048576)
) STRICT;
CREATE TABLE external_run_verifications (
 id TEXT PRIMARY KEY,run_id TEXT NOT NULL REFERENCES external_runs(id),
 created_at INTEGER NOT NULL,expires_at INTEGER NOT NULL,digest TEXT NOT NULL,
 payload TEXT NOT NULL CHECK(json_valid(payload) AND length(payload)<=1048576),
 executed_at INTEGER,receipt TEXT CHECK(receipt IS NULL OR (json_valid(receipt) AND length(receipt)<=32768))
) STRICT;
PRAGMA user_version=5;
