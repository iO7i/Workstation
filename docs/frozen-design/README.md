# Workstation — architecture and scope freeze FS-1.0 + D1

Design date: 18 September 2026. Working name only.

This is a **design pack**, not an installed or runnable product. It freezes one local Windows application with Health, Workspaces, Chronicle, Resource Atlas (including secrets-provider integration), and contextual Capabilities.

## Read in this order

1. `SYSTEM-DESIGN.md`: authoritative scope, architecture, safety rules, interfaces, threat model, release sequence, and primary sources.
2. `ACCEPTANCE-MATRIX.md`: the required positive and negative implementation tests.
3. `contracts/` and `examples/`: versioned reference envelopes and synthetic examples. They define shapes, not security authorization.
4. `schema/001_initial.sql`: executable relational starting point. The domain layer must implement authorization, temporal supersession, filesystem identity, and safe execution. A database constraint alone cannot make a repair safe.
5. `VALIDATION.md`: what was actually checked while preparing this design pack, versus tests still required on Windows.
6. `sources.json`: machine-readable primary-source register.

## Builder handoff

Implement R0 first, then the gates in section 18. Do not implement the entire roadmap in parallel. Reuse the user's existing D:\Ops/Docker experiments as evidence and audited adapters, not as trusted privileged scripts. Do not modify the user's running agents or production systems while building the tool.

The master document is normative; examples are illustrative. A conflict between a contract and a safety requirement must block the action and be reconciled before shipping. Unknown evidence, unsupported vendor versions, and denied access are first-class results, never implicit permission.

No secret values are included. All project names, process identities, resource references, and file hashes in the examples are synthetic. Test hashes are not signatures or authorization tokens.

## Scope changes

A new platform, cloud service, always-on watcher, autonomous fixer, full transcript store, or general secret manager requires a new explicit scope decision. None is an implicit TODO in v1.

## Accepted amendment — FS-1.0-D1

Contextual Resource Discovery is accepted and integrated into SYSTEM-DESIGN section 12 and the
R4/R5 gates. Read `FS-1.0-D1.md` and `FS-1.0-D1-ACCEPTANCE.md` alongside the base design. R0,
three-crate architecture, SQLite choice and mutation/secret/MCP boundaries are unchanged.

The reference database and original eight contracts remain the base design. D1 persistence and
runtime interfaces are implemented at R4; six additional draft authoring contracts and synthetic
policy fixtures are supplied with the source package, not silently applied as R0 migrations.
