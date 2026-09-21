# Implemented architecture

```
CLI command
  -> validate explicit own home
  -> open own SQLite metadata
  -> dispatch approved read-only collector to our own executable
       -> stdin request waits until process containment is attached
       -> bounded filesystem / native process / Git worktree observation
       -> typed JSON only
  -> validate collector response
  -> derive generic findings
  -> record at most twenty snapshots in own database
  -> private report OR independent share projection
```

`workstation-core` is OS-free and forbids unsafe Rust. It owns serializable evidence models, categorical coverage, simple diagnostic rules, entry-logical deltas, and report projection.

`workstation-platform` owns filesystem and Windows operations, the contained worker supervisor, Git invocation, and SQLite. Errors cross boundaries as fixed codes, not arbitrary logs. Unsafe code is restricted to native API bindings/owned process containment.

`workstation-cli` owns orchestration, envelopes, limits, and rendering selection. It does not add mutation features absent from the platform.

## Persistence

`config.json` binds an installation ID and explicit home. `roots` registers approved measurement targets; `projects` registers trusted repository/Git executable pairs; `scans` stores bounded typed snapshots. The schema has a distinctive application ID and version. No transcript/secret/content table exists.

Up to 24 non-overlapping roots and 20 projects. At most 20 scan snapshots, each at most 2 MiB. Own database limit is 32 MiB with default SQLite 4 KiB pages; refused persistence returns the current live snapshot with `saved=false`. Five completed backup+receipt pairs exhaust the initial backup budget; review is manual, not auto-deletion. WAL/SHM are own SQLite sidecars, not application state.

No database file from Codex, Cursor, Claude, or another product is opened. Backup uses the own database's backup API, integrity check and SHA-256 receipt, not a blind copy of a live `.db`.

## Collection budgets

Normal scan: 10-second orchestration target. Deep: 120-second overall default. Storage per-root budget: 700 ms / 20,000 entries / depth 32; deep 4 seconds / 100,000 entries / depth 64. Own worker protocol: request cap 64 KiB, stdout 2 MiB, stderr 16 KiB. Git: 1 MiB and 200 registered worktrees. Process selection: 1,000; enumeration cap 20,000. Collection is sequential initially. Pending-directory paths are capped at 2,048; omitted traversal becomes partial coverage rather than unbounded memory growth.

A budget exhaustion never means a complete zero-result scan. Native disk/own database operations still depend on OS progress; kernel stalls are not proven bounded here.

## Process boundary

The supervisor launches only the current Workstation executable. The child waits for request EOF; the parent establishes a private kill-on-close Windows Job Object before sending input. Both output streams are drained. A timeout/output flood terminates only this owned job. Normal completion also closes contained descendants. Every direct child is explicitly waited on. No existing agent is assigned to that job.

The Linux process-group path exists to execute portable tests, not to advertise Linux product support. Native process/disk collectors explicitly return `unsupported` there.

## Trust and privacy

Registered roots can be private, so normal reports are private. Share reports are a new schema omitting paths, user labels, branches, installation IDs, PIDs, argv, raw errors, and file contents. No HTTP library, telemetry, automatic updates, credentials, provider integrations, or raw transcript import exists in this slice.

All observed worktrees and processes are protected. `prunable` is a Git administrative observation, not permission to remove files. A missing parent PID is not proof of abandonment. No process lifecycle inference is offered.

## Evolution

R1 adds richer evidence and workspace protections without changing observation into authority. R2 can add reviewed typed plans only after negative fixtures pass. Do not add destructive command switches to this source slice merely because it already knows a path.

## Accepted FS-1.0-D1 addition — implement at R4

See `docs/scope/FS-1.0-D1.md` (repository-relative path) and `specs/discovery/v1/`.
Contextual resource discovery is now part of the v1 end-state but does not change this R0 slice.
Do not mistake `discover` (filesystem-root candidates) for Useful discoveries.
Finish native R0 verification before R4; keep the existing no-repair/no-network runtime boundary.
Do not copy synthetic catalog entries into a production reviewed catalog. R4 requires actual
resource reviews, typed Rust policy, scoped state and context/report integration; R5 requires
import, privacy, rendering, MCP, latency and unchanged-authority evidence.
