# Slice decision: R0-A — read-only baseline

This implementation selects the first vertical part of FS-1.0 R0. The full R0 release gate is not claimed until native compilation and Windows execution pass.

## User outcome

Answer, from bounded evidence: which approved storage roots are growing, which selected processes exist, which Git worktrees are registered, and what remains unknown. Produce a compact share-safe report without reading prompt/credential contents.

## Included / acceptance basis

| Component | Implementation | Gate |
|---|---|---|
| Home | Explicit `--home` / `WORKSTATION_HOME`; new-directory init; local fixed NTFS; restricted Windows home ACL | Missing home/drive does not materialize a C: fallback; existing directory not overwritten |
| Own metadata | Three small strict tables; schema identity; pinned bundled SQLite; bounded busy wait; backup API | Integrity/restore and contention on Windows |
| Storage | Metadata-only known/registered roots; limits; reparse/cloud skip; partial coverage | Canary not read into output; no accidental link traversal |
| Processes | Native ToolHelp + limited-query observations | Restricted visibility stays partial; no ownership/removal claims |
| Git | Explicit executable and project trust; stable NUL output | No file changes, fetch, checkout, helper hooks, or cleanup |
| Reports | Human, JSON, escaped static HTML, separate share projection | No raw contents/arguments/paths in share report |
| Collector lifecycle | Our executable only; stdin gate; private Windows Job Object; output/time bounds | Timeout/flood/descendant tests on actual Windows |
| Incidents | Six replay fixtures / three failure families | No mutation or live-diagnosis implication |

## Deliberately deferred from complete R0 / later stages

- Profile locator: use explicit home/environment first, avoiding profile mutation and competing installation state.
- Strong host/boot identity and session ownership edges: observations only, not inferred identity authority.
- Physical allocation/hard-link deduplication: show null allocation and entry-logical measurements.
- State-folder activity proof and proprietary agent session parsers: root candidates only.
- Dirty/untracked/ignored/unique-commit worktree analysis: all worktrees protected until implemented.
- Detailed vendor findings/native doctors and Docker endpoint validation: no command is run against Docker or WSL in R0-A.
- Restore CLI: consistent backups exist; programmatic fixture restore is tested by the eventual native suite. Operator restore documentation is manual and preserves the old home.
- Automatic schedule, hooks, home moves, decisions, resources, secrets, integrations, GUI, MCP: excluded.

This is intentional staging, not a change to the FS-1.0 end-state. Unknown coverage must not be patched over to make the report green.

## No-overengineering decisions

Three crates, three tables, no plugin framework, no async runtime, no network stack, no agent SDK, no service. A contained self-worker is justified because filesystem/Git operations can block; merely checking elapsed time in a recursive loop cannot bound a blocked system call.

## Safety caveats

Read-only refers to the inspected ecosystem. Own initialization, registration, scan retention, backup and build files are writes by design. The user-selected Git binary executes within a trusted registered repository; Workstation is not a sandbox against malicious same-user code. Metadata scanning can trigger OS/access-audit activity. No claim of a kernel-failure-proof deadline is made.

## Accepted FS-1.0-D1 addition — implement at R4

See `docs/scope/FS-1.0-D1.md` (repository-relative path) and `specs/discovery/v1/`.
Contextual resource discovery is now part of the v1 end-state but does not change this R0 slice.
Do not mistake `discover` (filesystem-root candidates) for Useful discoveries.
Finish native R0 verification before R4; keep the existing no-repair/no-network runtime boundary.
Do not copy synthetic catalog entries into a production reviewed catalog. R4 requires actual
resource reviews, typed Rust policy, scoped state and context/report integration; R5 requires
import, privacy, rendering, MCP, latency and unchanged-authority evidence.
