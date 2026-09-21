# Workstation 0.4.0-alpha.1 — implementation milestone, not a release

**IMPLEMENTED_SOURCE_UNVERIFIED. Windows and certification remain paused.**

This ZIP extends the exact `0.3.0-alpha.1` source; no parallel rewrite. Seven modules,
three Rust crates, local SQLite and 13 read-only MCP tools remain. No Windows device,
agent, provider account or production resource was accessed during authoring.

## What changed

| Module | New source paths and behavior |
|---|---|
| Health | Registered-profile scans, Windows process-birth/ancestry graph, managed-session and optional configured-hook bindings, version/freshness/contradiction-aware symptom rules. Unknown ownership stays protected. |
| Workspaces | Branch/file/Work Item/worktree collisions; protection and eligibility; explicitly approved non-force cleanup with retained HEAD; exact new-worktree creation through Git or optional pinned Worktrunk. |
| Chronicle | Claude/Gemini/Codex/ACP/Copilot lifecycle normalization and coalesced metadata ingestion; observations never become approved reasoning. |
| Atlas | Local encrypted secret versions, CAS rotation, local revoke, recovery inventory, freshness/status separation and explicit adoption drafts. DPAPI remains non-portable. |
| Capabilities | Cached installed-profile availability, existing-option preference, explicit Atlas/Chronicle drafts, revision-bound review; 27-entry reference catalog retained. |
| Economics | Copilot stdio SDK-wire quota/models, Claude configured statusline quota, Gemini session statistics, account-isolated caches; existing Codex and organization-cost readers retained. |
| Continuity | Gemini/OpenCode/Cline conditional ACP and Copilot new/resume added to Codex/Claude/Cursor/Grok. Roo/Windsurf remain packet-only. |
| Shared | Schema4, terminal-journal archive/hydration, archive-aware metadata backup/restore, bounded own storage, strict new operation contracts. |

## Read first

- [Implementation ledger](IMPLEMENTATION-LEDGER.md): same 52 coarse source families and explicit limits.
- [Current capabilities](CURRENT-CAPABILITIES.md): what the source does and does not implement.
- [v4 operational design](docs/IMPLEMENTATION-V4.md): ownership, lifecycle, recovery and safety.
- [Commands](docs/COMMANDS-V4.md): future operator workflow; examples are inert synthetic data.
- [Research](docs/RESEARCH-V4.md): current primary references and conditional transport boundaries.
- [Validation status](VALIDATION.md): no product or native tests were run.
- [Next builder](docs/NEXT-BUILDER.md): wait for the user's certification green light.

## The percentage is narrowly defined

The agreed source-family milestone tally is **48.5/52 = 93.27%**: 48 rows have scoped
source implementations, one has partial native-vendor coverage, three remain reference-only.
**This is not “93% of the product proven to work” or an estimate of engineering effort left.**
Broad historical labels are qualified in the ledger. No automatic full ownership of arbitrary
unconfigured agents, universal vendor diagnosis, portable DPAPI export, all-vendor quota API,
or generic tool installer is claimed.

## Safety and prerequisites

No external effect occurs from a scan, catalog card, resource manifest, cached context, or MCP call.
Writes require `effect prepare`, exact digest review, fresh revalidation and an explicit apply
acknowledgement. Worktree removal additionally needs complete supported observations and an
operator declaration that external writers have been coordinated. This is NOT an OS lock against
unmanaged agents or malicious same-user software. It never forces Git removal or deletes branches.

Process candidates remain protected; ended/expired sessions do not authorize kills or cleanup.
Unsupported native/provider methods fail explicitly or use the packet/import fallback. Quota,
context, credits and dollars remain different meters. Hook input is agent-reported metadata.

No executable or fabricated Cargo.lock is included. Source is not compiled/typechecked and may
contain defects. Exact direct dependency pins are inherited; the later build must resolve the real
graph, format, check, lint, test, verify Windows behavior and fix defects before use.

## Certification gate (do NOT run until authorized)

`scripts/build.ps1` and manual CI require explicit certification authorization. After the user's
separate green light and prerequisites are available, the documented entry point is:

```powershell
.\scripts\build.ps1 -CertificationAuthorized -ResolveDependencies -Format
```

That is a future build workflow, not evidence of a completed build. Follow docs/NEXT-BUILDER.md
and complete the new v4 acceptance cases, especially worktree removal, local secret versioning,
archive restore, owned process cleanup and actual native agent permission handling.

Historical evidence under `evidence/history` describes earlier versions only. Package/member
hashes establish archive integrity, not application correctness.
