# Builder handoff: complete native verification of this exact slice

Read README.md, SLICE.md and validation/VALIDATION.md first.

The archive contains implemented R0-A source, not a compiled/certified release. Do not expand scope to R1/R2 to avoid finishing its gates.

1. Extract onto a local D: directory. Preserve existing D:\Ops, Docker data/scripts/tasks, active agents, credentials and repositories.
2. Inventory existing Rust/MSVC prerequisites. Do not install software or globally relocate caches without explaining and obtaining the required permission.
3. Review the direct dependency pins. Run `scripts/build.ps1 -ResolveDependencies -Format` on Windows with the required toolchain. Generate the real Cargo.lock; never fabricate one.
4. Fix any compilation/lint/API/test defects at their source. Keep the gates enabled. In particular verify native ACLs, Job Object containment, child cleanup, Unicode/reparse behavior, SQLite runtime and backup.
5. Preserve raw command output and actual test counts. The included artifact tests are not Rust or Windows test passes.
6. Run the isolated smoke before touching a real application root. Then opt into read-only real-root/project registration. Report all partial coverage; do not terminate applications to improve a scan.
7. Validate a disconnected/missing chosen home, readonly/permission-denied entries, a linked worktree containing ignored credentials, and a legitimate shared Node/MCP process. All must remain protected.
8. Produce the actual compiled portable ZIP only after its gates pass; mark it unsigned until signing is real. Commit the lockfile and source. Keep the design's scope intact.

No process-kill user command, cleanup, Docker restart, Codex registry repair, secret resolver, scheduler, cloud service, or LLM dependency belongs in this slice.

## Accepted FS-1.0-D1 addition — implement at R4

See `docs/scope/FS-1.0-D1.md` (repository-relative path) and `specs/discovery/v1/`.
Contextual resource discovery is now part of the v1 end-state but does not change this R0 slice.
Do not mistake `discover` (filesystem-root candidates) for Useful discoveries.
Finish native R0 verification before R4; keep the existing no-repair/no-network runtime boundary.
Do not copy synthetic catalog entries into a production reviewed catalog. R4 requires actual
resource reviews, typed Rust policy, scoped state and context/report integration; R5 requires
import, privacy, rendering, MCP, latency and unchanged-authority evidence.
