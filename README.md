# Workstation 0.5.0-alpha.1

Workstation is a local-first Rust control plane for inspecting development environments and coordinating bounded work across supported coding agents. Version 0.5 adds a durable execution layer: run state survives process exits, progress is append-only, cancellation is revision-checked, and completion remains separate from verification and ownership release.

This is a public alpha candidate, not an unattended automation service. It does not install agents, collect credentials, expose writable MCP tools, or infer that a provider's “idle” state means a turn completed.

## Release status

- Windows 11 x64 is the primary runtime target; the public CI workflow targets Windows and Ubuntu.
- Rust 1.97.1 is pinned by the release gates.
- Schema 5 upgrades are backed up, transactional, idempotent, and preserve existing records.
- The full all-feature suite passes locally: 350 tests, including 17 Windows durable-runtime end-to-end tests with offline fake providers.
- The release binary is unsigned.
- Live-provider behavior is limited to separately recorded native evidence; fake-provider success is not live-provider certification.

See [CURRENT-CAPABILITIES.md](CURRENT-CAPABILITIES.md), [VALIDATION.md](VALIDATION.md), and [docs/DURABLE-RUNTIME-LIMITATIONS.md](docs/DURABLE-RUNTIME-LIMITATIONS.md) before using the alpha.

## Build and test

Prerequisites are Windows 11 x64, Visual Studio Build Tools with the MSVC x64 toolchain, PowerShell 5.1 or newer, and Rust 1.97.1 MSVC already installed.

```powershell
.\scripts\build.ps1 -CertificationAuthorized -TestScratchParent D:\WorkstationTests
```

Use `-Offline` when the pinned dependency graph is already cached. The build script runs formatting checks, all-feature compile and Clippy gates, all tests, a release build, and isolated smoke tests before packaging a ZIP and SHA-256 file under `dist\`.

For portable source checks:

```text
cargo +1.97.1 fmt --all -- --check
cargo +1.97.1 check --workspace --all-targets --all-features --locked
cargo +1.97.1 clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo +1.97.1 test --workspace --all-features --locked
```

## Durable runtime

Durable operations live under `workstation effect runtime`. Cached status, event, list, and bounded watch commands do not invoke a provider. Mutating commands use explicit revisions or approval digests.

```powershell
$W = '.\target\release\workstation.exe'
& $W --home D:\WorkstationHome --json effect runtime status --id RUN_ID
& $W --home D:\WorkstationHome --json effect runtime events --id RUN_ID --after 0 --limit 100
& $W --home D:\WorkstationHome --json effect runtime cancel --id RUN_ID --expected-revision REVISION
```

Baseline capture hashes approved workspace content and requires an explicit acknowledgement. Verification executes only pre-registered task IDs and checks exact allowed/forbidden paths. A passing verification does not mark a Work Item done and does not release its primary assignment.

The implementation guide is [docs/DURABLE-RUNTIME-IMPLEMENTATION.md](docs/DURABLE-RUNTIME-IMPLEMENTATION.md). Sanitized examples are in [examples/v5](examples/v5).

## Security and privacy

Do not attach Workstation homes, SQLite databases, provider transcripts, prompts, environment files, or credential material to public issues. Review [SECURITY.md](SECURITY.md) before reporting a vulnerability. Runtime output deliberately retains bounded metadata rather than prompt or credential values.

## License

MIT. See [LICENSE](LICENSE).
