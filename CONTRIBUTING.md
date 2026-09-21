# Contributing

Keep changes inside the issue or pull request scope, preserve unrelated work, and add a minimal sanitized regression fixture for behavior changes. Never include credentials, prompts, transcripts, local databases, private reports, or machine-specific paths.

Before opening a pull request, run:

```text
cargo +1.97.1 fmt --all -- --check
cargo +1.97.1 check --workspace --all-targets --all-features --locked
cargo +1.97.1 clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo +1.97.1 test --workspace --all-features --locked
```

On Windows, also run `scripts/build.ps1 -CertificationAuthorized` with a test scratch directory outside every Git checkout. Do not weaken a gate, regenerate `Cargo.lock` without review, make network/live-provider calls in tests, or treat fixture success as vendor certification.

Durable-runtime changes must preserve these invariants: no prompt replay during recovery, claim before spawn, append-only events, expected-revision mutation, exact session/turn reconciliation, bounded deadlines and storage, content-based verification, and no implicit work completion or ownership release.

Security reports belong in the private channel described in [SECURITY.md](SECURITY.md), not in a public issue.
