# FS-1.0-D1 implementation acceptance additions

All **28** cases below require R4/R5 product tests. None is a native/Rust/Windows pass in this delivery.
The coverage column names only the present specification/reference coverage; partial coverage is not certification.

| ID | Case | Required behavior | Current specification coverage |
|---|---|---|---|
| D1-001 | Relevant opportunity | Approved need and source evidence produce a contextual card; an unrelated project is quiet. | reference_policy |
| D1-002 | No inferred defect | Technology inventory without approved needs does not invent a task, deficiency or recommendation. | reference_policy |
| D1-003 | Existing equivalent | Prefer a relevant, applicable existing capability; shared need alone is not equivalence. | reference_policy |
| D1-004 | Unknown inventory | Partial/denied inspection yields unknown, not absent. | reference_policy |
| D1-005 | Three-card limit | At most three cards; do not fill empty results with weak or popular candidates. | reference_policy |
| D1-006 | Constraints and decisions | Known platform, privacy, price or accepted-decision conflicts suppress candidates. | reference_policy |
| D1-007 | Unknown compatibility | Unknown price/version/license/applicability is disclosed; never install-ready. | reference_policy_partial |
| D1-008 | Scoped dismissal | Dismissal remains local to project/need, including known canonical aliases and new catalog revisions. | reference_policy |
| D1-009 | Snooze and feedback order | Honor snooze expiry, latest explicit local feedback and timezone; ignore future records. | reference_policy |
| D1-010 | Offline and empty catalog | Offline normal context remains usable; empty catalog yields no invented matches. | reference_policy_partial |
| D1-011 | Review freshness | Stale review visible; no future review establishes current approval. | reference_policy |
| D1-012 | No adoption inference | Downloading/saving/adopting is not observed benefit or acceptance of a project decision. | reference_policy_partial |
| D1-013 | Scoped outcomes | Useful/not-useful/inconclusive trial applies only to exact project, need and reviewed resource revision. | reference_policy |
| D1-014 | Disclosed research brief | Default brief excludes private IDs/paths/secrets/history; explicit disclosure approval binds its digest. | schema_only |
| D1-015 | Candidate trust boundary | Strict bounded input; importer assigns IDs/unverified/reference-only locally; input cannot claim review or authority. | schema_only |
| D1-016 | Malicious text inert | Prompt injection, shell text, HTML and terminal controls stay escaped data; no execution/fetch/activation. | not_implemented |
| D1-017 | Import resource limits | Reject oversized/deep/invalid/duplicate payloads and interrupted writes without data loss. | schema_only |
| D1-018 | Read-only MCP | Cache/sanitized draft only; no import, feedback, plan approval, secret, installation or account tools. | not_implemented |
| D1-019 | Atlas/Chronicle separation | Feedback cannot silently create accepted decisions or replace chosen environment resources. | not_implemented |
| D1-020 | Integration revision change | Changed source/content/target invalidates approval; reference-only resource has no executable adapter. | not_implemented |
| D1-021 | Context latency | No search/network/scanner/model on cached context path; optional cards do not block diagnosis. | not_implemented |
| D1-022 | Resource-specific licensing | Mixed repository licensing is scoped to the selected resource; no repository-wide safety badge. | schema_only |
| D1-023 | Catalog curation | 20–30 actual initial resources have current review records and limitations; fixtures do not count. | not_implemented |
| D1-024 | R0 preservation | R0 Rust/build/applied schema/runtime contracts preserved; no live catalog or new runtime dependencies. | artifact_integrity |
| D1-025 | Canonical collision | Untrusted import cannot shadow reviewed resource, reset dismissal or bypass canonical deduplication. | reference_policy_partial |
| D1-026 | Own retention | Bound reviewed/unreviewed catalog/disposition storage without erasing approved history or requiring a daemon. | not_implemented |
| D1-027 | Reference only is useful | Docs, skills, datasets and benchmarks can be described without executable integrations. | reference_policy |
| D1-028 | Unavailable browsing | Denied/no online handoff remains a usable local workflow and never causes hidden network attempts. | not_implemented |

Before release record actual executable/adapter versions, host, scope, fixture digests, commands, observed effects,
network calls, context payload, elapsed time and limitations. Keep negative cases for active sessions, untrusted
imports and unchanged approved decisions. Schema tests alone cannot prove UI/MCP/import execution safety.
