# Source implementation ledger — 0.4.0-alpha.1

This is an implementation-only delivery. No compilation, application tests, Windows access, vendor execution or certification occurred.

## Meaning of the 93% target

Under the same 52-family **scoped milestone** bookkeeping: **48 source-implemented + one partial at half credit + three reference-only = 48.5 / 52 = 93.27%.**

This is not a measured percentage of the full product, engineering effort, correctness or readiness. Families are unequal. The earlier 93% forecast was not scientific. The old row labels are kept below; broad labels such as “universal certified” or “portable vault recovery” are **not** claims we implemented their literal unlimited meaning. The approved plan narrowed them to documented interfaces, conservative operations, and honest unsupported/recovery states. Those limits are written beside each row, not hidden as certification-only gaps.

The three reference-only families are Infisical/Bitwarden secret execution, generic automatic specialist activation, and a licensed network benchmark feed. The partially implemented family is additional-vendor native continuation: Gemini/Copilot/OpenCode/Cline have source transports; Roo/Windsurf still do not.

## Evidence semantics

`implemented_source` = concrete code wired to an interface within the stated scope, **not tested correctness**. `partial_source` = real missing code. `reference_only` = descriptive/import fallback, no external implementation. `verification_deferred` = a real acceptance demonstration, not a code family.

## Rows

### Health

| ID | Original feature family | Source status | Concrete code | Scope / remaining limitation |
|---|---|---|---|---|
| F01 | Bounded host/storage/process observations | implemented_source | `crates/workstation-platform/src/filesystem.rs`<br>`crates/workstation-platform/src/windows.rs`<br>`crates/workstation-cli/src/main.rs` | Native behavior is unverified; not every vendor/state path is inferred. |
| F02 | Registered executable and declared home inspection | implemented_source | `crates/workstation-platform/src/profile_health.rs`<br>`crates/workstation-cli/src/advanced_cli.rs`<br>`crates/workstation-platform/src/health_scan.rs` | Exact hash/selected metadata only; does not authenticate session ownership. |
| F03 | Birth-qualified ownership correlation | implemented_source | `crates/workstation-core/src/ownership.rs`<br>`crates/workstation-platform/src/process_graph.rs`<br>`crates/workstation-platform/src/operations_store.rs`<br>`crates/workstation-platform/src/health_scan.rs` | Observed native process births/ancestry plus owned protocol-session binding and optional configured-hook ancestry. Shared/unknown remain protected. Uninstrumented native sessions, loaded-image authenticity and full OS handle ownership are not inferred. |
| F04 | Typed symptoms and contradictory evidence | implemented_source | `crates/workstation-core/src/diagnostics.rs`<br>`crates/workstation-core/src/health_rules.rs`<br>`fingerprints/rules-v1.json` | Reported symptoms are not a certified exhaustive vendor failure corpus. |
| F05 | Scoped Codex corrupt-JSON registry quarantine | implemented_source | `crates/workstation-platform/src/repairs.rs`<br>`crates/workstation-platform/src/engine.rs` | Stopped-state invalid JSON only. Valid-but-stale/oversized registries and visual snippet verification remain outside this recipe. |
| F06 | Scoped Docker runtime quarantine, undo and separate startup | implemented_source | `crates/workstation-platform/src/repairs.rs`<br>`crates/workstation-platform/src/engine.rs` | No data reset, global WSL shutdown or retry loop. Source was not executed on real Windows sockets. |
| F07 | Universal certified first-class vendor diagnostics | implemented_source | `crates/workstation-platform/src/profile_health.rs`<br>`crates/workstation-platform/src/health_scan.rs`<br>`crates/workstation-core/src/health_rules.rs`<br>`fingerprints/rules-v1.json` | Source scope is the planned bounded registered-profile diagnostics for Codex/Claude/Cursor/Copilot and conditional protocol checks. The legacy title includes certification: NO universal vendor diagnosis, exhaustive private-state parser or certified defect corpus is claimed. Unknown evidence and unsupported versions are explicit. |

### Workspaces

| ID | Original feature family | Source status | Concrete code | Scope / remaining limitation |
|---|---|---|---|---|
| F08 | Git content-state and operation observations | implemented_source | `crates/workstation-platform/src/control_workspace.rs`<br>`crates/workstation-core/src/continuity.rs` | Dirty/untracked/ignored/local-only metadata stays protected, no cleanup permission inferred. |
| F09 | Work Item/session/workspace associations | implemented_source | `crates/workstation-platform/src/control_store.rs`<br>`crates/workstation-platform/src/integration_store.rs` | Lifecycle reports are advisory/agent-reported, not an OS-exclusive lock. |
| F10 | Pre-continuation identity/HEAD/state revalidation | implemented_source | `crates/workstation-platform/src/engine.rs`<br>`crates/workstation-core/src/continuity.rs` | Metadata-only digest can miss same-size same-mtime edits; receiver must inspect content/tests. |
| F11 | Work Item/workspace collision warnings | implemented_source | `crates/workstation-core/src/workspace_policy.rs`<br>`crates/workstation-platform/src/operations_store.rs` | Work Item, workspace, branch and file-path overlap; stale leases remain uncertain. File overlap uses Git changed paths/current reported scope, not prediction of future edits. Coordination warnings do not lock or terminate another agent. |
| F12 | Worktrunk workflow integration | implemented_source | `crates/workstation-platform/src/specialists.rs`<br>`crates/workstation-platform/src/workspace_lifecycle.rs` | Optional pinned schema2 local inventory and exact approved new-worktree creation with hooks disabled. Merge/rebase/background branch-deleting removal remain intentionally excluded; conservative removal uses native Git. |
| F13 | Safe general worktree cleanup | implemented_source | `crates/workstation-core/src/workspace_policy.rs`<br>`crates/workstation-core/src/operations.rs`<br>`crates/workstation-platform/src/workspace_lifecycle.rs`<br>`crates/workstation-platform/src/operation_runtime.rs` | An explicit non-force secondary-worktree operation only after fresh complete evidence, released work, explicit external-writer quiescence and HEAD preservation. No unattended general cleaner; no branch deletion. No same-user adversarial race-proof claim or automatic dirty-work recovery. |

### Chronicle

| ID | Original feature family | Source status | Concrete code | Scope / remaining limitation |
|---|---|---|---|---|
| F14 | Immutable decision proposal/acceptance/supersession | implemented_source | `crates/workstation-core/src/control.rs`<br>`crates/workstation-platform/src/control_store.rs`<br>`schema/002_control_plane.sql` | Unverified runtime wrapper; decisions are not auto-accepted from agents. |
| F15 | Effective-time and known-time queries | implemented_source | `schema/current_decisions.sql`<br>`crates/workstation-platform/src/control_store.rs` | Historical SQL tests belong to the prior version; current source not rerun. |
| F16 | History/compare/export interfaces | implemented_source | `crates/workstation-platform/src/project_tools.rs`<br>`crates/workstation-cli/src/advanced_cli.rs` | Bounded records; no automatic natural-language fact extraction. |
| F17 | Metadata lifecycle and execution audit ingestion | implemented_source | `crates/workstation-core/src/lifecycle.rs`<br>`crates/workstation-platform/src/integration_store.rs` | Claude/Codex/ACP metadata projection only; hooks require explicit user configuration. |
| F18 | General automatic event-source integration | implemented_source | `crates/workstation-core/src/lifecycle.rs`<br>`crates/workstation-platform/src/integration_store.rs`<br>`crates/workstation-cli/src/advanced_cli.rs` | Normalized Claude/Gemini hooks, Codex notifications, ACP and Copilot session events; CLI ingestion plus protocol-execution audit. Hooks must be explicitly configured. No silent hook deployment, background transcript ingestion or automatic decision acceptance. |

### Resource Atlas

| ID | Original feature family | Source status | Concrete code | Scope / remaining limitation |
|---|---|---|---|---|
| F19 | Explicit environment lookup and confirmation/freshness | implemented_source | `crates/workstation-platform/src/control_store.rs`<br>`crates/workstation-platform/src/tasks.rs`<br>`crates/workstation-platform/src/capability_context.rs` | Atlas status separates user confirmation, network observation and provider identity. No provider identity verifier implemented; no environment fallback. |
| F20 | Descriptive transactional manifest import | implemented_source | `crates/workstation-platform/src/project_tools.rs`<br>`crates/workstation-cli/src/advanced_cli.rs` | Does not silently replace chosen resources, confirm production authority or approve decisions. |
| F21 | Scope-bound local DPAPI vault | implemented_source | `crates/workstation-platform/src/vault.rs` | Native unverified; same-user programs are not isolated; metadata backup is not portable credential backup. |
| F22 | Doppler and 1Password exact secret resolution | implemented_source | `crates/workstation-platform/src/tasks.rs`<br>`crates/workstation-core/src/integrations.rs` | Pinned CLI, one key, scoped profile; bootstrap local DPAPI only; no provider administration. |
| F23 | Approved executable/script task with secret environment | implemented_source | `crates/workstation-core/src/integrations.rs`<br>`crates/workstation-platform/src/tasks.rs`<br>`crates/workstation-platform/src/external.rs` | Outputs discarded. Authorized program/imports can read/exfiltrate values; no full dependency sandbox. |
| F24 | Public HTTPS endpoint response observation | implemented_source | `crates/workstation-platform/src/http_provider.rs`<br>`crates/workstation-platform/src/engine.rs` | Public:443 HEAD only; no redirect. Not proof of provider ownership or production correctness. |
| F25 | Infisical and Bitwarden execution adapters | reference_only | `crates/workstation-core/src/control.rs` | Provider references exist; live secret resolution not implemented. |
| F26 | General rotation/revocation/portable vault recovery | implemented_source | `crates/workstation-platform/src/secret_lifecycle.rs`<br>`crates/workstation-platform/src/operations_store.rs`<br>`schema/004_lifecycle_completion.sql` | Planned scope is local encrypted version rotation, CAS head, local revoke/no legacy fallback, history and recovery inventory. DPAPI is NOT portable credential backup; restoring metadata can require secret reentry. Does not rotate/revoke the actual remote-provider token. |

### Capabilities

| ID | Original feature family | Source status | Concrete code | Scope / remaining limitation |
|---|---|---|---|---|
| F27 | Project focus and constraints-based local matching | implemented_source | `crates/workstation-core/src/discovery.rs`<br>`crates/workstation-platform/src/control_store.rs`<br>`crates/workstation-platform/src/capability_context.rs` | No native runtime test; unknown applicability stays unknown. |
| F28 | Three-card cap and existing-resource preference | implemented_source | `crates/workstation-core/src/discovery.rs` | Cached registered/pinned executable availability now informs preference. Existence is not compatibility or usefulness. Cached context does not launch live probes. |
| F29 | Scoped save/dismiss/snooze/evaluate/adopt | implemented_source | `crates/workstation-platform/src/control_store.rs`<br>`crates/workstation-core/src/discovery.rs`<br>`crates/workstation-platform/src/capability_context.rs` | Separate Atlas record and Chronicle proposal drafts are offered for reviewed revisions. No automatic acceptance, registration or installation from feedback. |
| F30 | Research brief and candidate import/review | implemented_source | `crates/workstation-core/src/discovery.rs`<br>`crates/workstation-platform/src/control_store.rs` | Unverified leads stay inert. No automatic search, watched skill writes or arbitrary installer. |
| F31 | 27-entry reference catalog | implemented_source | `catalog/resources.json`<br>`catalog/REVIEW-REGISTER.json` | Inherited existence/purpose review only; no new license/effectiveness/Windows certification claim. |
| F32 | Automatic specialist activation/integration certification | reference_only | `crates/workstation-core/src/integrations.rs` | No generic installation engine. Unsupported specialists remain descriptions. |

### Economics

| ID | Original feature family | Source status | Concrete code | Scope / remaining limitation |
|---|---|---|---|---|
| F33 | Separated meters and reset-aware forecasts | implemented_source | `crates/workstation-core/src/economics.rs` | Estimates remain estimates; providers with no usable quota remain unknown. |
| F34 | Plan fit, model Pareto/filters and explicit counterfactuals | implemented_source | `crates/workstation-core/src/economics.rs` | No automatic model/subscription switch or unlicensed benchmark redistribution. |
| F35 | Codex documented App Server quota read/persistence | implemented_source | `crates/workstation-platform/src/adapters.rs`<br>`crates/workstation-platform/src/project_tools.rs` | No real account exercised; imported responses and local received observations retain different provenance. |
| F36 | OpenAI/Anthropic organization cost HTTP readers | implemented_source | `crates/workstation-platform/src/http_provider.rs`<br>`crates/workstation-core/src/usage_import.rs` | Bounded admin API; org scope not project attribution; provider exclusions/delay not invoice truth. |
| F37 | Exact USD/cents normalization and overlap checks | implemented_source | `crates/workstation-core/src/usage_import.rs` | Nano-USD precision bound, rejects unsupported finer precision; authored tests unrun. |
| F38 | Copilot SDK and strict CSV fallback normalization | implemented_source | `crates/workstation-core/src/usage_import.rs`<br>`crates/workstation-cli/src/advanced_cli.rs` | Explicit import only; not a live SDK client. |
| F39 | Other live consumer quota/provider SDK collectors | implemented_source | `crates/workstation-platform/src/copilot.rs`<br>`crates/workstation-platform/src/telemetry_store.rs`<br>`crates/workstation-core/src/telemetry.rs`<br>`crates/workstation-core/src/usage_import.rs` | Adds real optional Copilot SDK-wire quota/models code and configured Claude statusline quota path; Gemini session token input is not subscription quota. Codex/organization readers retained. Unavailable Cursor/Grok/Gemini consumer quota endpoints remain unsupported/import-only; no scraping. No account calls executed. |
| F40 | Licensed benchmark network feed | reference_only | `crates/workstation-core/src/economics.rs` | Pluggable imported data/metadata remains; no licensed live Artificial Analysis feed integrated. |

### Continuity

| ID | Original feature family | Source status | Concrete code | Scope / remaining limitation |
|---|---|---|---|---|
| F41 | Work Items, checkpoints and bounded fallback packet | implemented_source | `crates/workstation-core/src/continuity.rs`<br>`crates/workstation-platform/src/control_store.rs` | No transcript transfer or semantic completion certification. |
| F42 | Primary/reviewer lease/reservation/binding lifecycle | implemented_source | `crates/workstation-platform/src/integration_store.rs`<br>`crates/workstation-platform/src/engine.rs` | Clean interaction release; failures retain uncertainty. Does not replace existing live primary. |
| F43 | Codex new/resumed App Server turn | implemented_source | `crates/workstation-platform/src/adapters.rs`<br>`crates/workstation-platform/src/rpc.rs` | Exact installed version/provider state untested; thread metadata is not automatically active session. |
| F44 | Claude print/JSON new and resumed interaction | implemented_source | `crates/workstation-platform/src/adapters.rs` | Permission mode and bounded turns; no native run or proven continuation outcome. |
| F45 | Cursor/Grok ACP new/load/prompt | implemented_source | `crates/workstation-platform/src/adapters.rs`<br>`crates/workstation-platform/src/rpc.rs` | Load/auth/plan mode conditional; permission callback metadata does not create OS sandbox. |
| F46 | Metadata hooks and explicit lease renewal | implemented_source | `crates/workstation-core/src/lifecycle.rs`<br>`crates/workstation-cli/src/advanced_cli.rs`<br>`crates/workstation-platform/src/integration_store.rs` | Preconfigured reports remain agent-reported; not writable MCP. |
| F47 | Other vendor native continuation | partial_source | `crates/workstation-platform/src/acp_driver.rs`<br>`crates/workstation-platform/src/copilot.rs`<br>`crates/workstation-platform/src/adapters.rs` | Gemini/OpenCode/Cline conditional ACP plus Copilot wire new/resume implemented. Roo and Windsurf remain packet-only because no applicable source-reviewed native transport was implemented. All native paths unexecuted. |
| F48 | Real multi-agent task acceptance | verification_deferred | — | Requires installed authenticated agents and user green light; never substituted with synthetic success. |

### Shared foundation

| ID | Original feature family | Source status | Concrete code | Scope / remaining limitation |
|---|---|---|---|---|
| F49 | Backed-up schema1/2→3 migrations and new-home restore | implemented_source | `crates/workstation-platform/src/integration_store.rs`<br>`crates/workstation-platform/src/storage.rs`<br>`schema/003_integrations.sql` | Source not executed; intermediate successful v2 migration can remain after v3 failure, explicitly reported. |
| F50 | Immutable exact plans and one-shot execution journal | implemented_source | `crates/workstation-core/src/effects.rs`<br>`crates/workstation-platform/src/engine.rs`<br>`crates/workstation-platform/src/integration_store.rs` | Same-user consent accounting, not human-authentication enforcement. Failed/interrupted effects not replayed. |
| F51 | Owned bounded process/RPC interaction | implemented_source | `crates/workstation-platform/src/external.rs`<br>`crates/workstation-platform/src/rpc.rs` | Windows Job Objects and pipe teardown untested. Docker GUI is an intentional persistent separate launch. |
| F52 | Cached project context, static reports and 13 read-only MCP tools | implemented_source | `crates/workstation-platform/src/control_store.rs`<br>`crates/workstation-cli/src/mcp.rs` | Optional context sections may be omitted to fit bound; MCP cannot apply plans or resolve secrets. |
| F53 | Long-term archive/retention UX for immutable effect journals | implemented_source | `crates/workstation-platform/src/journal_archive.rs`<br>`crates/workstation-platform/src/operations_store.rs`<br>`crates/workstation-platform/src/storage.rs`<br>`schema/004_lifecycle_completion.sql` | Explicit old terminal-run payload archive, digest-checked hydration, dependency-aware backup/restore, bounded packs and inventory. Running/indeterminate records are retained. No guaranteed physical file shrink, secure erasure, infinite log storage or silent deletion of authoritative history. |
