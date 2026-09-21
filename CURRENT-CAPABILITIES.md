# Current capabilities — 0.5.0-alpha.2

The current candidate adds durable execution without broadening automatic authority. The table distinguishes local test evidence from live-provider evidence.

| Surface | Implemented and locally verified | Boundary |
|---|---|---|
| Build | Rust 1.97.1; format, all-target/all-feature check, Clippy with warnings denied, and 350 tests | Clean-runner CI is configured but cannot be observed until the repository is published |
| Persistence | SQLite schema 5; transactional v4→v5 migration, pre-migration backup, idempotency, foreign-key/integrity checks, retained v4 records | Existing runs are not invented or replayed during migration |
| Durable runs | Persisted projection plus append-only event log; revision/sequence CAS; 512-run and 2,048-event budgets | No automatic retry and no event-log compaction in this alpha |
| Supervision | Claim is committed before spawn; child identity is birth-qualified; bounded handshake, session, idle, absolute, cancellation, and reconciliation deadlines | OS/process continuity can become uncertain and then requires reconciliation |
| Cancellation | Persisted request, expected-revision check, supervisor-scoped signalling; committed cancellation wins a timeout race | Cancellation is cooperative until the owned supervisor/child observes it |
| Recovery | Scans persisted nonterminal runs; reconciles local process facts without replaying a prompt | Provider reconciliation is separate and explicitly approved |
| Read paths | Status, list, paged events, and bounded watch are cached-only | Watch polls local SQLite for at most 300 seconds and emits at most 256 frames |
| Verification | Content-hash baseline, exact allow/forbid paths, pinned task IDs, task/executable content checks, stored receipt | Passing verification does not complete work or release ownership |
| MCP | Existing fixed read-only surface remains; durable mutations are CLI-only | No writable MCP runtime commands |
| Provider fixtures | 17 Windows end-to-end tests cover completion, crashes, deadlines, cancellation, reconciliation, hashing, and verification | Fixtures are offline and are not vendor certification |
| Live adapters | Prior native read-only/version evidence remains available under `evidence/` | Authentication, current vendor versions, and successful live task completion must be re-certified per environment |
| Packaging | Windows release ZIP, checksum, dependency inventory, copied third-party license files, and evidence bundle | Binary is unsigned; dependency inventory is not claimed as a standards-compliant SBOM |

Detailed semantics are in `docs/DURABLE-RUNTIME-IMPLEMENTATION.md`; known gaps are in `docs/DURABLE-RUNTIME-LIMITATIONS.md`.
