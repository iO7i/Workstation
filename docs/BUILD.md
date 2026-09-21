# Build and release gate

Workstation 0.5.0-alpha.2 pins Rust 1.97.1. The Windows package script requires an already installed `1.97.1-x86_64-pc-windows-msvc` toolchain and Visual Studio Build Tools; it does not install or modify either one.

Run from a 64-bit PowerShell process:

```powershell
.\scripts\build.ps1 -CertificationAuthorized -TestScratchParent D:\WorkstationTests
```

`TestScratchParent` must be an absolute local directory outside every Git repository. Each invocation creates a unique child directory. Build caches default to `.build\cargo` and `target`, but an existing process-scoped `CARGO_HOME` or `CARGO_TARGET_DIR` is respected.

Use `-Offline` only when every locked dependency and the advisory/build inputs required by the command are already cached. Use `-ResolveDependencies` only for a reviewed source tree with no `Cargo.lock`; release candidates must commit and review the resulting lockfile. `-Format` authorizes rustfmt to modify source; without it formatting is check-only.

For an offline RustSec vulnerability check, pass an existing local advisory database with `-AdvisoryDatabase D:\path\to\advisory-db`. The gate disables yanked-crate lookup because that requires a current crates.io index, and records that boundary in the build report.

The script stops on the first failure and runs:

1. formatting check;
2. all-target, all-feature compile check;
3. all-target, all-feature Clippy with warnings denied;
4. all-feature workspace tests, including the Windows durable fake-provider suite;
5. optimized CLI build;
6. isolated Windows baseline and control-plane smoke tests;
7. build-info capture, dependency inventory, third-party license collection, evidence copy, ZIP creation, and SHA-256 generation.

The ZIP is unsigned. The dependency inventory is not represented as a standards-compliant SBOM, and fixture success is not live-provider certification. Publishing maintainers must review `SECURITY.md`, `VALIDATION.md`, the generated manifest, checksum, and `evidence/runtime-v5-build-report.json` before creating a GitHub release.
