# Evidence index

`runtime-v5-build-report.json` is the current local build report for Workstation 0.5.0-alpha.1. It records the exact local gate categories, test totals, offline/network boundary, toolchain, RustSec result, and known release limitations without embedding credentials, prompts, transcripts, Workstation homes, or machine-specific paths.

The remaining top-level files describe earlier 0.4 candidates and are retained for provenance. Subdirectories named `history` or `historical`, and the separate `audit/` tree, are historical records. They must not be interpreted as current v5 certification.

The Windows release packager allowlists only `runtime-v5-build-report.json` into the public binary ZIP. Historical or machine-specific evidence is never copied wholesale into that artifact.
