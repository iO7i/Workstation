# Current capabilities — 0.3.0-alpha.1

**All rows describe source implementation, not certified runtime support.** Native/Rust,
provider, agent and repair executions for this delivery: **zero**. Testing on the user's
Windows machine is explicitly paused until the next green light.

| Surface | Implemented source | Not established / missing |
|---|---|---|
| Read-only baseline | Selected storage, process and Git collectors, conservative coverage, snapshots/reports | Windows collector/runtime behavior not exercised |
| Profile registration | Explicit project/environment, immutable executable hash/version declaration, allowed config paths, auth references | Version text is an operator claim; no blanket vendor support |
| Profile inspection | Bounded metadata checks and hash mismatch reporting, configured Codex/Claude roots | No full session/handle ownership discovery; editor paths not guessed from CLI |
| Ownership | Birth-qualified graph with complete/partial evidence, shared/expired cases, protected retained candidates | Input comes from declared evidence; automatic multi-vendor graph is incomplete |
| Workspaces | Dirty/untracked/ignored/local refs/operations, cache and continuation checks | No destructive cleanup; no content-equivalence proof from metadata |
| Chronicle | Immutable decision semantics, temporal history/compare/export, lifecycle/audit events | No automatic acceptance; event integrations need real provider confirmation |
| Atlas manifests | Strict project/environment scope, transactional import without replacing accepted resources | A manifest is not production authority |
| Local vault | User-scoped DPAPI, scope-bound encrypted payload, hidden input, explicit confirmation | Untested on Windows; no rotation/cloud sync/portable credential recovery workflow |
| Doppler/1Password | One exact scoped secret via pinned CLI; local DPAPI bootstrap; output withheld | No accounts exercised; no provider administration |
| Credentialed tasks | Pinned executable/script/cwd, exact resource bindings, revalidation, output discard | Child code can read/exfiltrate granted values; not a sandbox or human authentication |
| Endpoint verification | Approved public HTTPS:443 HEAD, bounded DNS/request, no redirect or credential attachment | HTTP response is not provider ownership or production correctness |
| Codex | App Server initialize, quota read, bounded thread metadata, new turn/resume | No live version/account; no private desktop session conversion |
| Claude | Metadata hooks/import, print/json new/resume; session ID bound before prompt | No real CLI; Stop is turn-end, not session-end; quota polling absent |
| Cursor/Grok | ACP initialize/auth negotiation, new/load-if-advertised, prompt, scoped permission handling | No live ACP; provider executes tools, so declared permissions do not create OS isolation |
| Gemini/Copilot/OpenCode/Cline/Windsurf | Explicit profiles/version path; packet/import fallback | Native continuation/health parity not implemented |
| D1 discoveries | Retained catalog/matcher/feedback/import review and cached context cards | No new catalog effectiveness/certification claim; no autonomous activation |
| D2 forecasts | Separate meters, horizons, resets, sparse data, plan fit, Pareto and scenarios | Exact personal quotas unavailable for some providers; no automatic routing |
| Organization costs | Bounded OpenAI/Anthropic admin GET, pagination, exact decimal nano-USD math | Organization scope, delayed/provider exclusions; not invoice or consumer subscription quota |
| Copilot/CSV input | Strict explicit import; unlimited/unknown handled without fabricated limits | Not an installed live Copilot SDK client |
| D3 continuation | Plan-bound packet/context, reserve/bind primary, turn/source IDs, clean release or uncertain protection | No real handoff; only four native transport families implemented |
| Repairs | Fresh stopped-state Codex corrupt-JSON/Docker runtime quarantine; same-volume rename, journal, verified-target undo | No active reset, valid-but-stale registry repair, universal cleanup, or root-cause-fix claim |
| Docker startup | Separate approved pinned GUI start; local Linux-engine probe, bounded observation | GUI intentionally persists, even if later probe fails; no kill/retry loop |
| Read-only MCP | Original 13 fixed project-scoped tools, bounded local context | No registration/apply/ownership/credential or network tools |
| Migration/backup | Explicit schema 1→2→3; version-3 aware restore to new home | Source not executed for this delivery |

Status labels are intentionally not binary “supports vendor.” See the capability function
in `workstation-core/src/integrations.rs` and `evidence/adapter-matrix.json` for per-operation
source coverage. All tested-version lists remain empty until actual evidence is recorded.
