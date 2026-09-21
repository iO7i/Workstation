> v0.4 update: current semantics and certification hold are in IMPLEMENTATION-V4.md,
> COMMANDS-V4.md and ../CURRENT-CAPABILITIES.md. The following section is preserved baseline context.

# Command guide — source candidate, native build required

Use `workstation --help` after compiling. Commands affect the chosen Workstation home
unless explicitly documented as read-only external inspection. No default C: home.

```powershell
$W = '.\target\release\workstation.exe'
$H = 'D:\Workstation-Test'
& $W --home $H init
& $W --home $H upgrade
& $W --home $H project add D:\ExampleRepo --id example `
  --git 'C:\Program Files\Git\cmd\git.exe' --trust-repository
& $W --home $H doctor
& $W --home $H context --project example
& $W --home $H context --project example --environment dev --html
& $W --home $H context --project example --share
& $W --home $H timeline --project example
& $W --home $H roster --project example
& $W --home $H capabilities --project example
```

## Typed metadata

`record --input <file>` validates shape and prints a digest preview without echoing
potentially private content. Review that file locally, then rerun with
`--approve-sha256 <digest>`. Inputs are at most 256 KiB; schemas are in contracts/control.
Examples under examples/requests are synthetic and assume project `example`.

Operations: environment_add, event_record, focus_set, economics_preference,
work_create/work_update, session_report, assign/release_assignment,
decision_propose/decision_accept/decision_dispose, resource_add/resource_confirm,
quota_import, feedback, candidate_import/candidate_review, plan_propose/plan_approve,
handoff_result. No generic SQL, shell or provider mutation operation exists.

Decision acceptance is an explicit separate record. Assignment expects the current
Work Item version; assignment increments it. Lease expiry never releases a primary.
Reported completion/tests do not certify an agent action. References carry no values.

## Continuity

```powershell
& $W --home $H workspace-register --project example --id main --path D:\ExampleRepo
# WorkCreate must name workspace_id=main before this checkpoint.
& $W --home $H checkpoint --work sample-task --input .\examples\checkpoint.json
& $W --home $H handoff --checkpoint '<returned-id>' --target grok
```

The handoff creates a packet, not a Grok session. It checks fresh metadata/HEAD/work
version, preserves dirty work, returns content-verification instructions and does not
transfer primary ownership. Inspect the repository/tests before acting on the packet.

## Economics — pure input calculations, no provider account needed

```powershell
& $W --json normalize-usage --format codex-rate-limits-2026-09 `
  --input .\fixtures\control\codex-rate-limits.json --version synthetic --account demo --observed-at 100
& $W --json model-frontier --input .\fixtures\control\models.json
& $W --json select-models --input .\fixtures\control\model-selection.json
& $W --json simulate --input .\fixtures\control\scenario.json
& $W --json plan-fit --input .\fixtures\control\plan-cycles.json
& $W --home $H economics --project example
```

`runway --input <QuotaSample array> --at <unix-seconds>` forecasts one comparable stream.
Do not use the whole economics-vectors test wrapper as the samples file. Copy a case's
`samples` array. Context, dollars and subscription remain separate. Normalized imports
are explicitly reported/partial; no browser cookie or authenticated UI is read.

## Read-only MCP

Start `workstation --home D:\Workstation-Test mcp --project example --environment dev`
with stdio. Configure this explicit executable/arguments in a supported client after
native testing. No auto-edit of AGENTS.md, CLAUDE.md or client MCP configuration.
No secret resolve, process termination, assignment or approval tool is exposed.

## Vault and backups

`vault-put --project example --environment dev --id token` requires native Windows
and a local console; no value argument or reveal command exists. The environment must
already be registered. It stores immutable user-scope DPAPI ciphertext and a reference.
Provider execution/injection is not enabled.

`backup` produces a SQLite metadata copy and receipt. `restore --backup <copy> --sha256
<receipt hash>` requires a **new** chosen home. Existing homes are not overwritten.
No archive extraction, vault decryption fallback or production access is part of restore.
