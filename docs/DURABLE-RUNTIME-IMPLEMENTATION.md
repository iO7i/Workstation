# Durable runtime implementation

Version 0.5 adds durable execution to the existing local Rust monolith. It does not add a daemon, cloud service, generic shell endpoint, writable MCP method, or automatic router.

## State and storage

Schema 5 adds `external_runs`, `external_run_events`, `external_run_baselines`, and `external_run_verifications`. A run has a materialized projection for bounded reads and an append-only event stream for evidence. Projection revision and event sequence advance in the same immediate SQLite transaction. Updates use compare-and-swap semantics; cancellation and other mutations reject stale revisions.

The v4→v5 migration first writes an isolated SQLite backup, then applies the schema in one transaction. It does not manufacture historical runs. Existing project and control-plane records are retained. Re-running the migration at schema 5 is a no-op.

## Execution lifecycle

Preparation pins the effect plan, project/environment, integration digest, executable hash/version, Work Item version, checkpoint, workspace identity, and deadline policy. Applying the plan transactionally reserves the effect and records the supervisor identity before any provider child is spawned.

The supervisor records transport and provider transitions, exact provider session/turn identifiers, birth-qualified process identities, heartbeats, and unique progress. Heartbeats and repeated chatter do not count as progress and do not grow the event log indefinitely. Provider `idle` is not treated as turn completion.

There is no automatic retry. Recovery scans persisted nonterminal runs and reconciles local process facts without sending another prompt. A provider read is a separate, exact-session/turn, approval-bound operation.

## Deadlines and cancellation

Handshake, session creation, idle, absolute runtime, cancellation grace, and reconciliation each have bounded policies. The absolute deadline remains a ceiling regardless of progress. Cancellation is persisted before supervisor signalling and requires the caller's expected revision. Only the run's recorded owned supervisor/child can be signalled. If cancellation commits before a racing deadline observation, cancellation remains authoritative.

## Verification

A baseline must be captured before execution and requires explicit acknowledgement that approved workspace content will be hashed. File contents are not retained. Verification contracts contain exact repository-relative allowed paths, forbidden paths, and up to eight pre-registered task IDs.

Execution rechecks the task definition and executable content, compares the current content manifest with the baseline, rejects changes outside the contract, and stores a bounded receipt. Mtime and file size are not accepted as content evidence. A passing receipt changes only the durable run's verification state; it does not mark the Work Item done or release the primary assignment.

## Operator surface

`workstation effect runtime` provides cached `status`, `list`, `events`, and bounded `watch` operations plus explicit `cancel`, `recover`, `reconcile`, `baseline`, `verify`, and `detach` operations. Durable mutations are intentionally absent from MCP.

The public examples under `examples/v5/` contain no executable paths, credentials, prompts, or environment-specific identifiers.
