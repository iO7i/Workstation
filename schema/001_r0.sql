-- R0 production schema. The FS-1.0 full-design schema is a reference, not applied here.
PRAGMA application_id = 1465078832;
CREATE TABLE roots (
    id TEXT PRIMARY KEY CHECK(length(id) BETWEEN 1 AND 64),
    path TEXT NOT NULL UNIQUE,
    kind TEXT NOT NULL CHECK(kind IN ('codex','claude','cursor','vscode','docker','workspace','other'))
) STRICT;
CREATE TABLE projects (
    id TEXT PRIMARY KEY CHECK(length(id) BETWEEN 1 AND 64),
    path TEXT NOT NULL UNIQUE,
    git_executable TEXT NOT NULL
) STRICT;
CREATE TABLE scans (
    run_id TEXT PRIMARY KEY,
    observed_at TEXT NOT NULL,
    snapshot_json TEXT NOT NULL CHECK(json_valid(snapshot_json) AND length(snapshot_json) <= 2097152)
) STRICT;
CREATE INDEX scans_by_time ON scans(observed_at DESC);
PRAGMA user_version = 1;
