# D1 package amendment validation — 18 September 2026

**Runtime:** unchanged R0-A source candidate. **D1 product:** not implemented. **Native release:** not certified.

## Executed for this amendment

- **44/44 development-only D1 tests passed**, zero failures, errors or skips.
- Six strict JSON authoring contracts and synthetic examples validated.
- Pure Python reference-policy cases exercised matching, constraints, three-card limits,
  existing-equivalent preference, unknown inventory, stale/future reviews, scoped feedback,
  snoozing, canonical deduplication and revision-specific usefulness.
- Negative JSON tests rejected several forbidden authority/execution/private fields,
  non-HTTPS or credential-bearing references, overlong fields and excessive candidates.
- **41 original non-document/non-validation artifacts are byte-identical**
  to the delivered first-slice archive. This includes every Rust source/test, Cargo manifest,
  compiler declaration, native build/smoke/CI file, applied R0 schema and R0 contract/fixture.
- Original source artifact/reference tests and full-design artifact tests were re-run separately.
  See artifact-checks.json and docs/frozen-design/validation-results.json.

Reproduce the D1 tests with `python scripts/validate_discovery.py` in a separate Python
3.11+ development environment with jsonschema. This Python helper is not a product dependency.
Raw results: discovery-spec-checks.json and discovery-spec-checks.log.

## Not established by those checks

- No Rust resource matcher, live context integration, feedback database, importer/exporter,
  catalog loader, MCP tool, renderer integration, installer or new user command was implemented.
- No real catalog entries were reviewed. The example.org records are synthetic fixtures;
  the R4 target of 20–30 reviewed resources remains required work.
- All **28 D1 product acceptance cases are not run**. A shape validator cannot prove import
  security, confidentiality, approval binding or renderer/MCP behavior.
- Rust compilation/tests, the native Windows executable and Windows integration remain
  unverified, as recorded in the baseline validation. The container has no rustc/cargo path.
- No external source retrieval, installation, agent restart, account connection, credential
  access, scheduled task, production modification or user-worktree mutation was performed.

## Scope outcome

FS-1.0-D1 is accepted into the design and builder handoff, implemented at R4 and certified
at R5. R0 remains the read-only baseline. `discover` still means filesystem-root candidates,
not Useful discoveries. No automatic authority is gained by this amendment.
