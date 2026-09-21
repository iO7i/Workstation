# Slice 2 research application

Primary references are recorded in SLICE2-SOURCES.json, plus catalog/REVIEW-REGISTER.json.
This pass checked documented Codex App Server rate-limit shapes, Claude statusline fields,
MCP initialization/stdio, DPAPI user scoping, Git status/ref queries and SQLite backup.

Implementation consequences: provider payloads are explicit imports with downgraded
reported provenance; cost totals are not subscription debits or invoices; quotas are
separate windows; no authenticated website/cookie scraping. MCP is a fixed read-only
stdio surface. Handoffs use Workstation checkpoints, not vendor hidden-state conversion.

All 27 catalog entries contain original short descriptions and links, not copied source
instructions, benchmark scores or datasets. Review establishes source existence/purpose,
not a consumer environment's license, safety, version compatibility or usefulness.

No current provider or integration was certified merely because a documentation page
exists. Missing account/Windows/executable capability is explicitly represented in the
adapter matrix. Linux Python SQLite used by tests is separate from the unbuilt Rust
application's eventual bundled SQLite.
