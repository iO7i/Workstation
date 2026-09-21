# Design-pack validation

**Result:** 37 artifact checks passed. These validate the design artifacts, not the proposed product.

Checked 8 JSON Schemas and synthetic examples, rejection of selected disallowed fields/actions, executable SQLite schema creation, relational constraints, immutable record guards, three illustrative temporal queries, source-reference consistency, and example completeness. Detailed results: `validation-results.json`. Reproduce with `python validate_design.py` after installing the Python `jsonschema` package in a separate test environment.

The SQL was exercised in an in-memory database using the container's SQLite **3.46.1**. That test does **not** certify this runtime for production, enable WAL, or satisfy the frozen patched-SQLite release requirement. Application release validation must check its actual bundled SQLite version/source ID separately.

The schemas validate structure, not confidentiality or authority. Rejecting a `value` field does not detect every secret that could be embedded in another string. The temporal queries demonstrate the reference ledger shape, not a completed conflict resolver or authorized decision service.

## Not executed here

No Windows application was compiled or run. No actual process was inspected or stopped, no Docker/WSL runtime repaired, no Codex state edited, no Git worktree removed, no DPAPI encryption/decryption performed, no credential accessed, and no vendor/provider integration certified. The 62 cases in `ACCEPTANCE-MATRIX.md` remain implementation requirements, not reported passes.

The master design is the frozen build specification. The SQL and contracts are reference starting points; the builder must implement the domain invariants, security gates, capability tests, and real Windows acceptance evidence before release.

## Accepted D1 amendment

FS-1.0-D1 is an accepted scope addition scheduled for R4/R5, not an implemented product.
The 28 cases in FS-1.0-D1-ACCEPTANCE.md remain **not run**. Their contracts, synthetic
examples and development-only reference-policy tests live under specs/discovery/v1 in the
source package. They are reported separately in validation/D1-VALIDATION.md.
R0 runtime code and applied schema were not changed by this amendment.
