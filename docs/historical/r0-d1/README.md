# Workstation — R0 read-only baseline

**Version:** `0.1.0-alpha.1` · **Design basis:** FS-1.0 + FS-1.0-D1 · **Delivery:** source candidate

One local command to inspect registered AI-workstation storage, selected processes, and Git worktree registrations—without trying to clean up your machine.

## Delivery status: read this before running

This archive contains implemented Rust source, tests, contracts, fixtures, Windows build automation, and evidence. **It does not contain a compiled Windows executable.** The authoring environment had neither a Rust compiler nor package-server connectivity. Rust compilation, Cargo dependency resolution, Rust tests, and Windows API execution could not be performed there.

Twenty independent artifact/reference-recipe checks were actually executed. They validate JSON contracts, SQLite schema/backup/retention recipes, and a read-only Git worktree command against temporary Linux repositories. They are **not substitutes for running the Rust application**. See `validation/VALIDATION.md` and the raw JSON/log results.

`Cargo.lock` is intentionally absent, not invented. The first native build explicitly resolves the reviewed direct dependency pins, creates a real lockfile, runs checks, and packages the executable only if the gates succeed. This source candidate is not a certified R0 release.

## Why this slice

Before a tool can safely repair or remember your workstation, it must make bounded observations, preserve unknowns, identify its own storage, and avoid exposing private contents. This is the first coherent vertical slice—not five unfinished feature modules.

### Implemented in source

- Explicit new home on a selected local NTFS drive; no silent C: fallback.
- Three Rust crates; one CLI; no resident service or network API.
- Own SQLite metadata with schema/runtime checks, twenty saved observations, size bounds, and online backup.
- Metadata-only storage scans with file/depth/time/output budgets; redirects/cloud placeholders skipped.
- Windows selected-process snapshots: names, PIDs, creation timestamps, working set, private commitment. Ownership is **unknown**, all selected processes **protected**.
- Registered repositories inspected using `git worktree list --porcelain -z` with an explicit trusted Git executable.
- Every worktree remains **protected**. Registration is not inspection of dirty/untracked/ignored files or local-only commits.
- Generic findings for low free space, incomplete storage coverage, and observed growth—not speculative vendor-bug diagnoses.
- Timestamped human/JSON reports, script-free private HTML, and a separate allowlisted share report.
- Six clearly synthetic incident inputs based on three reported failure families; replay never changes a workstation.
- Child collector containment, stdin gate, bounded output, deadlines, and explicit wait/reaping.

### Not in this slice

Repairs, cleanup, process termination of user applications, active-session ownership, full worktree safety analysis, secrets/DPAPI, Chronicle, resource catalogs, MCP, scheduled tasks, Docker/WSL restarts, native vendor doctors, or automatic migration of C: data. These remain future FS-1.0 stages. No placeholder is advertised as functional.

## Accepted addition: contextual resource discovery (R4, not this runtime)

The attached proposal is now tracked as **FS-1.0-D1**. It extends Capabilities into
**Useful discoveries**: selective project-contextual suggestions for existing features,
tools, skills, workflows, documentation, reference implementations, datasets and benchmarks.
It includes an offline reviewed catalog, scoped dismissal/snoozing, and optional reviewed
research handoff/import—never automatic installations or new authority.

**R0 remains unchanged.** The existing `discover` command discovers filesystem-root candidates;
it is not this feature. Live discovery, feedback persistence, imports and context integration
are scheduled for R4, with certification in R5.

Read [the accepted addendum](docs/scope/FS-1.0-D1.md),
[28 required product cases](docs/scope/FS-1.0-D1-ACCEPTANCE.md),
and [the specification assets](specs/discovery/v1/README.md).
The complete amended design is included in `docs/frozen-design/`.
Development-only contracts/reference tests are separate from native application validation.
See `validation/D1-VALIDATION.md` for actual results and preservation checks.

## Build on Windows

Use Windows 11 x64, an installed `1.97.1-x86_64-pc-windows-msvc` toolchain with rustfmt/clippy, and the Visual C++/Windows SDK build prerequisites. The script installs nothing and does not elevate. See `docs/BUILD.md`.

Extract the source to a local directory on D:, then:

```powershell
.\scripts\build.ps1 -ResolveDependencies -Format
```

The explicit switches allow initial dependency resolution and formatting. Build caches and temporary files are scoped to this checkout/drive. The existing installed Rust toolchain remains where you installed it. The script runs `check`, `clippy`, tests, a release build, and isolated Windows smoke checks before producing `dist\...zip`.

Subsequent builds use the real generated `Cargo.lock`:

```powershell
.\scripts\build.ps1
```

Commit the lockfile and formatted source. CI deliberately refuses an absent lockfile.

## First use after the build passes

No administrator shell is required. `D:\Workstation` must not already exist and its parent must exist.

```powershell
$ws = (Resolve-Path '.\target\release\workstation.exe').Path
& $ws --home D:\Workstation init
& $ws discover
```

`discover` only suggests known-root candidates. It does not know that a running GUI uses a shell's `CODEX_HOME`, and it does not register or recursively scan those candidates.

Register only existing roots you choose. Do not register a parent and its child:

```powershell
& $ws --home D:\Workstation root add --id codex-home `
  --path "$env:USERPROFILE\.codex" --kind codex

& $ws --home D:\Workstation root add --id docker-data `
  --path D:\Docker\Data --kind docker
```

Repository inspection requires explicit trust in the selected Git executable/repository. Replace the example repository with your actual path:

```powershell
$git = (Get-Command git.exe).Source
& $ws --home D:\Workstation project add D:\Repos\my-project `
  --id my-project --git $git --trust-repository
```

For this PowerShell session, `$env:WORKSTATION_HOME = 'D:\Workstation'` can replace repeating `--home`; no user-wide environment change is required.

Everyday commands:

```powershell
& $ws --home D:\Workstation doctor
& $ws --home D:\Workstation --json doctor
& $ws --home D:\Workstation doctor --deep
& $ws --home D:\Workstation report --share
& $ws --home D:\Workstation backup
```

`doctor` only writes Workstation's own metadata. `doctor --no-save` skips recording the observation, but opening the own database may still maintain SQLite sidecars. Inspected application files are not intentionally written.

PowerShell 5.1 file redirection may use UTF-16. Write HTML explicitly as UTF-8:

```powershell
$html = & $ws --home D:\Workstation report --html
[IO.File]::WriteAllText('D:\Workstation\reports\latest.html',
  ($html -join [Environment]::NewLine), (New-Object Text.UTF8Encoding($false)))
```

Private reports contain local paths/branches/process identifiers. Only `report --share` constructs the narrower export. Review even that export before publishing; product kinds, sizes, counts, and observation time are intentionally retained.

## Output and exit codes

- `0`: requested operation completed; warnings may still be present.
- `2`: blocked operation or a finding reached the explicit `--fail-on` threshold.
- `3`: inspection was partial/unsupported; inspect `coverage`, not just the return code.
- `4`: invalid command input or missing home selection.

`--json` emits one versioned envelope on stdout. Cached reports preserve their original observation time and include `CACHED_NOT_LIVE`.

`logical_entry_bytes` is an entry sum, not allocated disk space. Hard links can be counted more than once. A partial sum is a lower bound. `allocated_bytes` is deliberately null. Process private commitment is not presented as unique physical RAM.

## Repository map

```text
crates/workstation-core/       Types, findings, coverage, safe report projection
crates/workstation-platform/   Windows, paths, storage scan, Git, SQLite, runner
crates/workstation-cli/        CLI, collector protocol, integration tests
schema/                       Applied R0 schema only
contracts/                    Strict first-slice output/input contracts
fixtures/                     Synthetic incidents and illustrative reports
scripts/                      Build, Windows smoke, artifact validation
validation/                   What actually ran; raw evidence
docs/                         Scope, architecture, sources, build/handoff
```

No repairs or secrets are hiding behind undocumented flags. `__collector` is an internal protocol entry point; its debug fixture probes are disabled in release builds. It is not an arbitrary shell endpoint.

## Coexistence

Leave existing `D:\Ops`, Docker recovery scripts, startup tasks, agent conversations, credentials, and project tooling unchanged. Workstation is not required for them to work. A failed Workstation scan must not become an excuse to restart them.

MIT license for this project. The working name is not a trademark clearance claim.
