# Workstation 0.2.0-alpha.1 — independent Slice 2 source candidate

**Status: PARTIAL. Source implementation, not a certified Windows release.**

This package continues the selected **R0 + D1** baseline. It adds the independently
implementable control-plane layers requested in the seven-module execution brief.
The latest user instruction authorizes development without waiting for unavailable
Windows certification. It does **not** turn unrun gates into passes.

**Read first:** [Current capabilities](CURRENT-CAPABILITIES.md),
[validation](VALIDATION.md), [next builder](docs/NEXT-BUILDER.md),
[command guide](docs/COMMANDS.md).

## Seven modules; one SQLite-backed Rust application

| Module | Source implemented here | Important remaining boundary |
|---|---|---|
| Health | Original read-only collectors plus typed symptom classification, contradiction/freshness checks and diagnostic reports | No certified vendor-specific live collectors or external repair executor |
| Workspaces | Git registration plus dirty/untracked/ignored/local-only commit evidence, blockers and cached observations | No deletion; metadata is not byte-identical source verification |
| Chronicle | Immutable proposals/acceptances, bitemporal supersession, conflicts, bounded events | No default transcript ingestion |
| Resource Atlas | Explicit environments, resources, references, scoped lookup, user confirmations, Windows DPAPI implementation | DPAPI native tests unrun; provider resolution/credentialed execution not enabled |
| Capabilities | 27 reference entries; deterministic project focus, feedback, offline suggestions, research briefs and reviewed imports | Source-purpose review only, not 27 certified integrations |
| Economics | Separate meters, documented-format import normalization, runway, completed-cycle plan fit, Pareto/filtering and explicit simulations | Synthetic/imported observations; no live account streams |
| Continuity | Work Items, sessions, primary/reviewer roster, leases, collisions, checkpoints and fresh packet-only handoff | No vendor session launch/native resume or automatic primary transfer |

The normal context packet and fixed read-only stdio MCP use this same data model.
They do not run a scan, contact providers, install anything or resolve secret values.

## Delivered verification

The new executable test-host suite performs **116 checks**: 56 on the shipped SQL,
12 on actual Git recipes in temporary Linux repositories, 28 on a separate synthetic
economics oracle/shared vectors, and 20 JSON-contract/catalog/static checks.
These **do not execute the Rust binary**. Actual counts and environment are in
`evidence/control-tests.json`; Rust/Windows/live-provider execution is zero here.

There is **no `.exe` and no fabricated Cargo.lock**. This environment lacks Rust,
PowerShell, MSVC and a Windows host. Source may still contain compile/platform
issues; formatting, Clippy, all Rust tests and Windows smoke are mandatory next gates.

## Native build — existing Windows toolchain required

Extract outside any live project workspace, preferably onto D:. Build/test scratch
must be outside every repository. No global PATH/TEMP changes or installers run.

```powershell
.\scripts\build.ps1 -ResolveDependencies -Format
```

This retains the real dependency-lock bootstrap, formatting, check, Clippy and Rust
tests, and adds an isolated control-plane smoke gate before Windows packaging.
No passing synthetic/reference test suppresses a native failure.

## First local usage, only after the native gates pass

```powershell
$W = '.\target\release\workstation.exe'
& $W --home D:\Workstation-Test init
& $W --home D:\Workstation-Test upgrade
& $W --home D:\Workstation-Test project add D:\ExampleRepo `
  --id example --git 'C:\Program Files\Git\cmd\git.exe' --trust-repository
& $W --home D:\Workstation-Test context --project example
```

An existing R0 home is opened and **explicitly upgraded**, not deleted. A verified
metadata backup is made before schema migration. Do not test first on your live
Workstation home or production repositories.

Metadata changes use typed input with a preview digest:

```powershell
& $W --home D:\Workstation-Test record --input .\examples\requests\work-create.json
# Review the file locally. Then repeat with the exact preview digest:
& $W --home D:\Workstation-Test record --input .\examples\requests\work-create.json `
  --approve-sha256 '<exact preview digest>'
```

Approval applies to Workstation metadata, not a production operation or vendor
mutation. Same-user software can invoke the CLI; this is not an OS security sandbox.

## Independent verification available without Rust

In an already prepared Python 3.11+ development environment with jsonschema 4.26.0:

```text
python scripts/verify_control.py
python scripts/synthetic_demo.py
```

Python is a verification tool, **not a runtime dependency or a replacement application**.
The demo labels its SQL/reference-generated data synthetic; it does not claim that
Codex or Grok ran, a model was billed, or the Rust application executed.

## Safety limits

No generic shell runner, process killer, WSL shutdown, worktree deletion, automatic
provider switch or MCP mutation is exposed. Docker/Codex plans are persisted but
execution remains blocked until native recipes are certified. The existing D:\Ops,
Docker recovery automation, user repositories and credentials were not modified.

DPAPI protects at rest for the current Windows user. It does not isolate credentials
from malicious same-user programs. Metadata backups exclude vault ciphertext and are
not portable credential backups. Arbitrary user-entered prose still needs disclosure
review; simple canary/pattern rejection is not comprehensive secret detection.

`docs/frozen-design` and `docs/historical` preserve earlier design history. The latest
seven-module brief and current capability matrix take precedence over obsolete
five-module/R0-only descriptions. Old R0/D1 preservation validators are historical,
not current Slice 2 acceptance gates; see `docs/SCOPE-PRECEDENCE.md`.
