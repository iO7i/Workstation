# Validation status — Workstation 0.5.0-alpha.1

**Disposition: locally validated public alpha candidate.** This is evidence for the checked-out source on 21 September 2026, not a claim about an unpublished GitHub commit or every live provider.

## Passed gates

- Repository base: named branch `feature/durable-runtime`, base commit `59fa1b6b152aadd93d56ca3b7a03944d8e6f7fe7`; no Git remote is configured.
- Rust toolchain: `1.97.1-x86_64-pc-windows-msvc`.
- `cargo fmt --all -- --check`.
- `cargo check --workspace --all-targets --all-features --locked --offline`.
- `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`.
- `cargo test --workspace --all-features --locked --offline`: 350 passed, 0 failed.
- Durable runtime end-to-end suite: 17 passed, 0 failed, using only the repository's offline protocol fixture.
- Schema-v5 migration test: v4 data retained, backup created, integrity and foreign keys valid, required tables present, second upgrade is a no-op.
- Release build and Windows smoke/package gate: recorded in `evidence/runtime-v5-build-report.json` after the packaging command completes.

## Durable cases exercised

The end-to-end suite covers cancel-before-claim, live cancellation, supervisor crash recovery without prompt replay, fast terminal events, missing terminal reconciliation, separate handshake/session/idle/absolute deadlines, chatter and duplicate-progress suppression, exact session/turn matching, baseline timing, same-size/restored-mtime content changes, protected test files, task/executable pin invalidation, and a verified completion path.

Unit tests additionally cover event budgets, duplicate turns, path constraints, deadline validation, terminal matching, and provider state interpretation. Existing control-plane, storage, workspace, protocol, privacy, process, and adapter tests remain in the same all-feature gate.

## Not certified by these results

- GitHub Actions on a clean public runner.
- Authenticode signing or installer behavior.
- Current authenticated live-provider execution.
- macOS support or production Linux support.
- Resistance to a malicious same-user process or compromised operating system.
- Automatic work completion, ownership release, cross-agent routing, or retry; these are intentionally absent.

The machine-readable local report is `evidence/runtime-v5-build-report.json`. Historical evidence under `audit/` and `evidence/history/` remains historical and must not be read as a current release claim.
