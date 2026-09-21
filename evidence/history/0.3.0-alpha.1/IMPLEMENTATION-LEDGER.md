# Implementation ledger — 0.3.0-alpha.1

**Source implementation, not measured 93% completion or a tested Windows release.**

Previous 93% was a subjective planning estimate, not a measured or weighted feature denominator. Families below are not equally sized and cannot be averaged into product completion.

Each source path below is present in the package. That is evidence of implementation work, not proof of correctness. `partial_source`, `reference_only` and `not_implemented` mark real source/integration gaps; do not relabel them all as certification gaps.

## Health

| Capability | Source status | Code | Boundary / remaining work |
|---|---|---|---|
| Bounded host/storage/process observations | `implemented_source` | `crates/workstation-platform/src/filesystem.rs`<br>`crates/workstation-platform/src/windows.rs`<br>`crates/workstation-cli/src/main.rs` | Native behavior is unverified; not every vendor/state path is inferred. |
| Registered executable and declared home inspection | `implemented_source` | `crates/workstation-platform/src/profile_health.rs`<br>`crates/workstation-cli/src/advanced_cli.rs` | Exact hash/selected metadata only; does not authenticate session ownership. |
| Birth-qualified ownership correlation | `partial_source` | `crates/workstation-core/src/ownership.rs`<br>`crates/workstation-cli/src/advanced_cli.rs` | Correlates explicit evidence. Full automatic multi-vendor observation/handle attribution is not implemented. All candidates protected. |
| Typed symptoms and contradictory evidence | `implemented_source` | `crates/workstation-core/src/diagnostics.rs` | Reported symptoms are not a certified exhaustive vendor failure corpus. |
| Scoped Codex corrupt-JSON registry quarantine | `implemented_source` | `crates/workstation-platform/src/repairs.rs`<br>`crates/workstation-platform/src/engine.rs` | Stopped-state invalid JSON only. Valid-but-stale/oversized registries and visual snippet verification remain outside this recipe. |
| Scoped Docker runtime quarantine, undo and separate startup | `implemented_source` | `crates/workstation-platform/src/repairs.rs`<br>`crates/workstation-platform/src/engine.rs` | No data reset, global WSL shutdown or retry loop. Source was not executed on real Windows sockets. |
| Universal certified first-class vendor diagnostics | `not_implemented` | — | Per-version full coverage, automatic private-state parsers and certified repair fingerprints still need independent implementation/evidence. |

## Workspaces

| Capability | Source status | Code | Boundary / remaining work |
|---|---|---|---|
| Git content-state and operation observations | `implemented_source` | `crates/workstation-platform/src/control_workspace.rs`<br>`crates/workstation-core/src/continuity.rs` | Dirty/untracked/ignored/local-only metadata stays protected, no cleanup permission inferred. |
| Work Item/session/workspace associations | `implemented_source` | `crates/workstation-platform/src/control_store.rs`<br>`crates/workstation-platform/src/integration_store.rs` | Lifecycle reports are advisory/agent-reported, not an OS-exclusive lock. |
| Pre-continuation identity/HEAD/state revalidation | `implemented_source` | `crates/workstation-platform/src/engine.rs`<br>`crates/workstation-core/src/continuity.rs` | Metadata-only digest can miss same-size same-mtime edits; receiver must inspect content/tests. |
| Work Item/workspace collision warnings | `partial_source` | `crates/workstation-core/src/continuity.rs`<br>`crates/workstation-platform/src/integration_store.rs` | Work Item/workspace primary conflicts implemented; comprehensive branch/file-level conflict analysis is not. |
| Worktrunk workflow integration | `reference_only` | `crates/workstation-core/src/integrations.rs` | Explicit profile/version discovery exists; create/merge/remove backend not implemented. |
| Safe general worktree cleanup | `not_implemented` | — | No worktree removal command. Protection is intentional until reliable dependency/unique-work coverage and reviewed operations exist. |

## Chronicle

| Capability | Source status | Code | Boundary / remaining work |
|---|---|---|---|
| Immutable decision proposal/acceptance/supersession | `implemented_source` | `crates/workstation-core/src/control.rs`<br>`crates/workstation-platform/src/control_store.rs`<br>`schema/002_control_plane.sql` | Unverified runtime wrapper; decisions are not auto-accepted from agents. |
| Effective-time and known-time queries | `implemented_source` | `schema/current_decisions.sql`<br>`crates/workstation-platform/src/control_store.rs` | Historical SQL tests belong to the prior version; current source not rerun. |
| History/compare/export interfaces | `implemented_source` | `crates/workstation-platform/src/project_tools.rs`<br>`crates/workstation-cli/src/advanced_cli.rs` | Bounded records; no automatic natural-language fact extraction. |
| Metadata lifecycle and execution audit ingestion | `implemented_source` | `crates/workstation-core/src/lifecycle.rs`<br>`crates/workstation-platform/src/integration_store.rs` | Claude/Codex/ACP metadata projection only; hooks require explicit user configuration. |
| General automatic event-source integration | `partial_source` | `crates/workstation-core/src/lifecycle.rs` | No full all-vendor hook deployment, deployment observer or arbitrary transcript ingestion. |

## Resource Atlas

| Capability | Source status | Code | Boundary / remaining work |
|---|---|---|---|
| Explicit environment lookup and confirmation/freshness | `implemented_source` | `crates/workstation-platform/src/control_store.rs`<br>`crates/workstation-platform/src/tasks.rs` | No production-to-dev fallback. A user confirmation is not provider-identity verification. |
| Descriptive transactional manifest import | `implemented_source` | `crates/workstation-platform/src/project_tools.rs`<br>`crates/workstation-cli/src/advanced_cli.rs` | Does not silently replace chosen resources, confirm production authority or approve decisions. |
| Scope-bound local DPAPI vault | `implemented_source` | `crates/workstation-platform/src/vault.rs` | Native unverified; same-user programs are not isolated; metadata backup is not portable credential backup. |
| Doppler and 1Password exact secret resolution | `implemented_source` | `crates/workstation-platform/src/tasks.rs`<br>`crates/workstation-core/src/integrations.rs` | Pinned CLI, one key, scoped profile; bootstrap local DPAPI only; no provider administration. |
| Approved executable/script task with secret environment | `implemented_source` | `crates/workstation-core/src/integrations.rs`<br>`crates/workstation-platform/src/tasks.rs`<br>`crates/workstation-platform/src/external.rs` | Outputs discarded. Authorized program/imports can read/exfiltrate values; no full dependency sandbox. |
| Public HTTPS endpoint response observation | `implemented_source` | `crates/workstation-platform/src/http_provider.rs`<br>`crates/workstation-platform/src/engine.rs` | Public:443 HEAD only; no redirect. Not proof of provider ownership or production correctness. |
| Infisical and Bitwarden execution adapters | `reference_only` | `crates/workstation-core/src/control.rs` | Provider references exist; live secret resolution not implemented. |
| General rotation/revocation/portable vault recovery | `not_implemented` | — | Local values use immutable resource identity. Full lifecycle/admin/portable recovery workflows not implemented. |

## Capabilities

| Capability | Source status | Code | Boundary / remaining work |
|---|---|---|---|
| Project focus and constraints-based local matching | `implemented_source` | `crates/workstation-core/src/discovery.rs`<br>`crates/workstation-platform/src/control_store.rs` | No native runtime test; unknown applicability stays unknown. |
| Three-card cap and existing-resource preference | `implemented_source` | `crates/workstation-core/src/discovery.rs` | No popularity-only ranking or mandatory extra dependencies. |
| Scoped save/dismiss/snooze/evaluate/adopt | `implemented_source` | `crates/workstation-platform/src/control_store.rs`<br>`crates/workstation-core/src/discovery.rs` | A disposition is not installation authority or a Chronicle acceptance. |
| Research brief and candidate import/review | `implemented_source` | `crates/workstation-core/src/discovery.rs`<br>`crates/workstation-platform/src/control_store.rs` | Unverified leads stay inert. No automatic search, watched skill writes or arbitrary installer. |
| 27-entry reference catalog | `implemented_source` | `catalog/resources.json`<br>`catalog/REVIEW-REGISTER.json` | Inherited existence/purpose review only; no new license/effectiveness/Windows certification claim. |
| Automatic specialist activation/integration certification | `reference_only` | `crates/workstation-core/src/integrations.rs` | No generic installation engine. Unsupported specialists remain descriptions. |

## Economics

| Capability | Source status | Code | Boundary / remaining work |
|---|---|---|---|
| Separated meters and reset-aware forecasts | `implemented_source` | `crates/workstation-core/src/economics.rs` | Estimates remain estimates; providers with no usable quota remain unknown. |
| Plan fit, model Pareto/filters and explicit counterfactuals | `implemented_source` | `crates/workstation-core/src/economics.rs` | No automatic model/subscription switch or unlicensed benchmark redistribution. |
| Codex documented App Server quota read/persistence | `implemented_source` | `crates/workstation-platform/src/adapters.rs`<br>`crates/workstation-platform/src/project_tools.rs` | No real account exercised; imported responses and local received observations retain different provenance. |
| OpenAI/Anthropic organization cost HTTP readers | `implemented_source` | `crates/workstation-platform/src/http_provider.rs`<br>`crates/workstation-core/src/usage_import.rs` | Bounded admin API; org scope not project attribution; provider exclusions/delay not invoice truth. |
| Exact USD/cents normalization and overlap checks | `implemented_source` | `crates/workstation-core/src/usage_import.rs` | Nano-USD precision bound, rejects unsupported finer precision; authored tests unrun. |
| Copilot SDK and strict CSV fallback normalization | `implemented_source` | `crates/workstation-core/src/usage_import.rs`<br>`crates/workstation-cli/src/advanced_cli.rs` | Explicit import only; not a live SDK client. |
| Other live consumer quota/provider SDK collectors | `not_implemented` | — | No real live Cursor/Copilot/Gemini/Grok/Claude consumer quota collector; unsupported APIs not invented. |
| Licensed benchmark network feed | `reference_only` | `crates/workstation-core/src/economics.rs` | Pluggable imported data/metadata remains; no licensed live Artificial Analysis feed integrated. |

## Continuity

| Capability | Source status | Code | Boundary / remaining work |
|---|---|---|---|
| Work Items, checkpoints and bounded fallback packet | `implemented_source` | `crates/workstation-core/src/continuity.rs`<br>`crates/workstation-platform/src/control_store.rs` | No transcript transfer or semantic completion certification. |
| Primary/reviewer lease/reservation/binding lifecycle | `implemented_source` | `crates/workstation-platform/src/integration_store.rs`<br>`crates/workstation-platform/src/engine.rs` | Clean interaction release; failures retain uncertainty. Does not replace existing live primary. |
| Codex new/resumed App Server turn | `implemented_source` | `crates/workstation-platform/src/adapters.rs`<br>`crates/workstation-platform/src/rpc.rs` | Exact installed version/provider state untested; thread metadata is not automatically active session. |
| Claude print/JSON new and resumed interaction | `implemented_source` | `crates/workstation-platform/src/adapters.rs` | Permission mode and bounded turns; no native run or proven continuation outcome. |
| Cursor/Grok ACP new/load/prompt | `implemented_source` | `crates/workstation-platform/src/adapters.rs`<br>`crates/workstation-platform/src/rpc.rs` | Load/auth/plan mode conditional; permission callback metadata does not create OS sandbox. |
| Metadata hooks and explicit lease renewal | `implemented_source` | `crates/workstation-core/src/lifecycle.rs`<br>`crates/workstation-cli/src/advanced_cli.rs`<br>`crates/workstation-platform/src/integration_store.rs` | Preconfigured reports remain agent-reported; not writable MCP. |
| Other vendor native continuation | `not_implemented` | — | Gemini/Copilot/OpenCode/Cline/Windsurf native resume/continuation missing; packet fallback only. |
| Real multi-agent task acceptance | `verification_deferred` | — | Requires installed authenticated agents and user green light; never substituted with synthetic success. |

## Shared foundation

| Capability | Source status | Code | Boundary / remaining work |
|---|---|---|---|
| Backed-up schema1/2→3 migrations and new-home restore | `implemented_source` | `crates/workstation-platform/src/integration_store.rs`<br>`crates/workstation-platform/src/storage.rs`<br>`schema/003_integrations.sql` | Source not executed; intermediate successful v2 migration can remain after v3 failure, explicitly reported. |
| Immutable exact plans and one-shot execution journal | `implemented_source` | `crates/workstation-core/src/effects.rs`<br>`crates/workstation-platform/src/engine.rs`<br>`crates/workstation-platform/src/integration_store.rs` | Same-user consent accounting, not human-authentication enforcement. Failed/interrupted effects not replayed. |
| Owned bounded process/RPC interaction | `implemented_source` | `crates/workstation-platform/src/external.rs`<br>`crates/workstation-platform/src/rpc.rs` | Windows Job Objects and pipe teardown untested. Docker GUI is an intentional persistent separate launch. |
| Cached project context, static reports and 13 read-only MCP tools | `implemented_source` | `crates/workstation-platform/src/control_store.rs`<br>`crates/workstation-cli/src/mcp.rs` | Optional context sections may be omitted to fit bound; MCP cannot apply plans or resolve secrets. |
| Long-term archive/retention UX for immutable effect journals | `partial_source` | `schema/003_integrations.sql`<br>`crates/workstation-platform/src/storage.rs` | Hard own-DB budget exists; no full archive/rotation interface for permanent new effect records. |

## Certification hold

No Rust/native application tests, live provider/agent operations or Windows certification were performed this delivery. Only source authoring/review and package-integrity operations were performed. Wait for the user’s next green light before that phase.
