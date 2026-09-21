# FS-1.0-D1 — R4 discovery specification assets

**Status:** approved scope; reference artifacts only. Not an R0 module, plugin, migration,
reviewed live catalog, runtime command, or certified integration.

- `contracts/`: strict JSON authoring contracts for catalog metadata, private context,
  local feedback, bounded recommendation cards, minimal research briefs and untrusted imports.
- `examples/`: synthetic examples only. `example.org` links are test data, not recommendations.
- `reference_policy.py`: small development-only executable policy specification. It accepts
  already schema-validated synthetic data, has no network, OS mutation, subprocess, database,
  installation, MCP server or provider integration. R4 must implement the real typed Rust
  policy and import/persistence/rendering boundaries. Do not ship Python as a runtime dependency.
- `acceptance.json`: full R4/R5 implementation cases; status `not_run_product`.

Run `python scripts/validate_discovery.py` for actual specification checks. They do not
establish that Workstation has discovery, that a listed resource exists, that rendering/import
is secure, or that Rust/Windows tests passed. The native R0 code and applied R0 schema are unchanged.

Normal R4 context uses only a reviewed local catalog. Untrusted research leads live in a
separate review queue until a local review promotes bounded metadata. Supplying a valid JSON
shape never grants trust. Schema validation is necessary, not sufficient for privacy or safety.
