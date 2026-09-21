# Phase 1 audit package

Start with `AUDIT.md`; use `findings.json` for the 23 patched source findings and
`open-items.json` for 9 remaining review/certification gates. `COMPATIBILITY.md`
contains the effect-policy and input-format changes. `source.patch` is the source
change set; `baseline-documents/` preserves the original top-level claims.

Reproduce local-only evidence (Python 3.11+, Git and jsonschema required):

```text
python audit/run_checks.py
python scripts/verify_control.py --output audit/evidence/patched-control-tests.json
```

The audit harness tests actual Git/SQLite recipes, independent references and static
source guards; it does NOT run the Rust code. The original-archive hash test may skip
when the old ZIP is not present in a downstream checkout; its successful original
run is recorded here. These checks do not grant Windows certification permission.

Run native build, Rust tests, DPAPI, provider/handoff/repair certification only after
the user's separate green light. Keep all source/test failures visible. Do not
regenerate old results as if they applied to the new revision.
