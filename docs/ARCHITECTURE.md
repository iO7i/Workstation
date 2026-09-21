> v0.4 update: current semantics and certification hold are in IMPLEMENTATION-V4.md,
> COMMANDS-V4.md and ../CURRENT-CAPABILITIES.md. The following section is preserved baseline context.

# Current implementation architecture

Seven modules share one Rust monolith and one local SQLite metadata model. There remain
three crates: core (deterministic domain/policy), platform (typed effects/persistence), CLI
(human/JSON and fixed read-only MCP). No second orchestrator, daemon, database service or GUI
framework has been introduced.

```
Human CLI / explicit hook reports / read-only MCP
  -> project, Work Item, environment, evidence and current decisions
  -> cached context / workspace protection / discoveries / economics
  -> explicit typed plan (only on operator request)
  -> digest + expiry + scope + identity + state revalidation
  -> execution journal
  -> provider CLI / documented RPC / fixed HTTP / exact quarantine / approved task
  -> bounded result, or uncertainty without automatic retry
```

## New source seams

- core/integrations: registered capabilities, path/auth policies, pinned task definitions.
- core/effects: exact action enum, expiry/approval validation, typed receipt states.
- core/ownership: birth-qualified declared evidence, shared-owner/protection handling.
- core/lifecycle: vendor event metadata projection, not transcript import.
- core/usage_import: precise provider money/quota formats and CSV fallback.
- platform/integration_store: migration3, profile/task/plan/run/step persistence and reservations.
- platform/external + rpc: bounded owned process interaction; no public generic runner.
- platform/adapters: Codex/Claude/Cursor/Grok and narrow provider query transports.
- platform/tasks + http_provider: environment-scoped secrets, approved tasks, costs, public HEAD.
- platform/repairs: exact stopped-state quarantine/undo and independent Docker GUI startup.
- platform/engine: one prepare/apply/revalidation/journal path for every external effect.
- platform/project_tools: descriptive manifests, history/compare, authoritative-context hash.
- platform/profile_health: worker-bound registration and declared home metadata inspection.
- cli/advanced_cli: explicit operator surface. MCP remains read-only and does not dispatch here.

No method name implies native certification. Some interfaces remain provider-dependent,
provisional or unimplemented; the implementation ledger is part of this architecture.
