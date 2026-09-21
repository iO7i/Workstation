# Current capabilities — source vs observed execution

Version 0.2.0-alpha.1. Overall **PARTIAL_SOURCE_CANDIDATE**.
No native application capability has been certified in this environment.

| Surface | Implemented source | Actual evidence here | Status |
|---|---|---|---|
| Existing R0 collectors | Preserved plus bounded teardown improvement | Baseline source audited; no native execution | Windows gate outstanding |
| Explicit database upgrade | Backed-up transactional v1 -> v2 | Shipped SQL upgrade/rollback/FK/concurrency/backup tests | SQL tested; Rust wrapper unrun |
| Work Item/session/assignment | Explicit records, version CAS, one primary, manual release, partial reports | Actual SQLite constraints/concurrency | Rust flow unrun |
| Chronicle | Immutable revisions, explicit acceptance, time-aware supersession | Exact SQL query exercised for future/retro/conflict cases | SQL tested |
| Atlas | Explicit environment and scoped provider-neutral references | SQL cross-environment/FK tests | No live verification |
| DPAPI vault | User-scope protect/unprotect and hidden console entry | Code and native test source only | Windows unsupported here |
| Provider credentials | Doppler/1Password/Infisical/Bitwarden references | No accounts contacted | Reference-only |
| Git workspace snapshots | Read-only status/ignored/untracked/local refs and metadata digest | Same Git recipes on disposable Linux fixtures | Native runner untested |
| D1 discovery | 27 metadata-reviewed entries and Rust selection/feedback/import logic | Catalog/schema validation; authored Rust policy tests | Runtime untested |
| D2 economics | Meter-separated forecasts, plan fit, model frontier/filter and scenarios | 28 Python-reference/vector cases, no provider data | Synthetic-only |
| Codex usage format | Explicit-import normalizer for documented rate-limit shape | Synthetic shape/vector validation; Rust tests authored | No live App Server connection |
| Claude usage format | Explicit-import normalizer for documented statusline shape | Synthetic fixtures; cost remains estimated | No hooks installed |
| Other quota providers | No scraped/private API or guessed adapter | No provider interfaces exercised | Unsupported/reference-only |
| Continuity | Checkpoints, metadata freshness, packet-only, advisory roster/collisions | SQL/tests source; no real agents | Packet source implemented; native unrun |
| Vendor resume/launch/ACP | No fake parity | None | Unsupported pending certification |
| Read-only MCP | 13 fixed tools, version negotiation, bounded lines, fixed project/environment | Protocol/CLI Rust tests authored; static boundary checks | Native unrun |
| Repair plans | Typed target/digest/expiry/policy checks and audit | SQL constraints/tests source | Executor deliberately blocked |
| Synthetic project example | SQL and independent Python reference, JSON/HTML | Actual script execution | Demonstration, not application output |

A reported session/usage/test status does not become observed merely because it entered
SQLite. CLI approval records user intent but does not authenticate a human against an
unrestricted same-user agent. No code-signing, real benchmark license, real subscription
stream, real cross-agent transfer or vendor-version certification is implied.
