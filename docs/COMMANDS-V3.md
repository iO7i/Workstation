> v0.4 update: current semantics and certification hold are in IMPLEMENTATION-V4.md,
> COMMANDS-V4.md and ../CURRENT-CAPABILITIES.md. The following section is preserved baseline context.

# Added CLI surface — operator reference for the next authorized phase

**Do not execute these against the user's Windows machine until they give their next green light.**
This document records the implemented command paths; it is not a report that they ran.
All placeholders refer to a disposable future test home/repository, not production.

| Command family | Operation | Effect classification |
|---|---|---|
| `integration register --input FILE` | Return typed input digest; repeat with exact `--approve-sha256` to record | Workstation metadata only |
| `integration list --project ID` | Cached registered profile/capability summary | Local read |
| `integration capabilities --adapter codex` | Compiled source capability claims and unsupported operations | No host/provider probe |
| `integration inspect --id ID` | Bounded metadata/hash inspection through an owned worker | Local read; stores observation |
| `task register --input FILE` | Preview/record pinned task definition | Metadata only; does not run |
| `task list --project ID` | Registered tasks | Local read |
| `effect prepare --project P --environment E --input FILE` | Prepare exact expiring plan | May inspect local targets, no requested external effect |
| `effect show --id ID` | Review stored plan and its digest | Local read |
| `effect apply --id ID --approve-sha256 HASH --acknowledge-uncertified-execution` | Execute only approved variant after revalidation | Explicit external operation |
| `effect history --project ID` | Show receipts/uncertainty | Local read |
| `lifecycle import ... --input FILE` | Preview/record bounded vendor metadata event | Local metadata; no transcript import |
| `lifecycle hook --integration ID --format FORMAT --accept-agent-report` | Project-scoped metadata from bounded stdin | Agent-reported write; not MCP |
| `lifecycle renew --assignment ID --session ID --seconds N` | Preview/renew exact lease with digest | Local metadata; cannot replace primary |
| `chronicle history/compare/export` | Bounded temporal history/decisions | Local read |
| `manifest --input FILE` | Descriptive manifest preview, repeat with exact digest to import | Local metadata; no confirmation/decision granted |
| `ownership --input FILE` | Correlate declared birth-qualified evidence | No host collection or process action |
| `usage-import --format FORMAT --input FILE` | Normalize explicit cost/quota/CSV shapes | Import output only; no live calls |

Use `--help` after the native build is authorized for exact required arguments. Existing
`record`, `context`, `checkpoint`, `handoff`, `mcp`, `doctor`, `workspace`, `upgrade`, backup
and restore commands retain their typed scope. Legacy `record` Plan* rows remain historical
metadata plans; new operational plans are managed by the **effect** family.

## Examples

`examples/v3/` contains six profile templates, a pinned-script task, a descriptive manifest,
and typed effect requests. Hashes, IDs and paths are synthetic placeholders. Do not convert
placeholder strings into trusted records. Actual file identities, exact version claims,
scoped resource confirmations and human review are prerequisites for the future execution phase.

All profiles/tasks are immutable revisions. Use a new ID after a binary/script/version
changes; the executor rejects hash drift rather than silently trusting an update.

The CLI's display adapter name `onepassword` maps to JSON adapter `one_password`.
OpenAI cost requests use JSON provider `open_ai_costs`. JSON schema files are authoring aids;
Rust validation and live state checks are authoritative when the application is running.
