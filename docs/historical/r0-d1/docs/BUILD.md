# Native build and verification

## Source delivery boundary

No Rust compiler, cargo, network dependency resolution, Windows host or MSVC linker was available in the authoring container. The archive therefore contains source and genuine artifact checks, not a built executable or lockfile. Do not rename this source delivery to a certified release.

## Prerequisites

Use Windows 11 x64 and a 64-bit PowerShell terminal. Install/reuse the Rust MSVC toolchain `1.97.1-x86_64-pc-windows-msvc` with `rustfmt` and `clippy`, and Microsoft's C++ build tools/Windows SDK. The SQLite C amalgamation is bundled by the Rust dependency and needs the native compiler. Existing Git is only needed for Git integration tests and repository registration.

The build script will not install these prerequisites or bypass a failed check. Toolchain installation/build dependencies can consume disk; select appropriate drives through their supported installers. Keep this source checkout on D: to keep `.build`, package cache, target outputs and scratch files there. The script restores process environment variables on exit and does not change user-wide configuration.

Official references are in RESEARCH.md. Exact direct versions are pinned. The `bundled` SQLite runtime is checked at runtime; the actual version/source ID is reported by the executable after compilation.

## First build

```powershell
.\scripts\build.ps1 -ResolveDependencies -Format
```

Explicitly creates the first real Cargo.lock and allows rustfmt to format the source. Dependency retrieval requires network access on that build machine. Review and commit the generated lockfile and formatting changes.

The script gates packaging on:

1. pinned compiler present;
2. real Cargo.lock present or explicitly resolved;
3. formatting check;
4. all-target source check;
5. Clippy with warnings denied;
6. Rust debug tests, including contained-worker failure cases;
7. release binary build;
8. isolated Windows smoke against a fresh temporary home;
9. actual runtime/dependency inventory and hashes.

Any failure stops the build. An integration/API/compiler issue is a source defect to fix and retest, never a reason to disable the safety gate.

## Existing lockfile

```powershell
.\scripts\build.ps1
```

No unlocked build fallback. CI is intentionally red until the real reviewed Cargo.lock is committed. The workflow is authored here; it has not been run in this delivery.

## Output

`dist\workstation-r0-<timestamp>.zip` contains the compiled executable, actual lockfile, manifest, project license, dependency metadata, and top-level third-party license copies found by the script. Dependency metadata is not claimed to be a standards-compliant SBOM. Complete publishing/license review and signing remain separate release tasks.

The build is unsigned unless an actual signing process is added and verified. Do not disable Defender/SmartScreen or run as Administrator to mask a problem.

## Portable tests

A Linux developer can use the same compiler and `cargo test --workspace --locked` for portable code and runner tests. Windows collectors return unsupported on Linux. Passing Linux tests is not Windows certification.

## Optional independent artifact checks

```text
python scripts/validate_artifacts.py
```

Requires Python 3.11+ and jsonschema in a development environment only. These checks intentionally distinguish SQL/Git reference recipes from executing the Rust implementation.
