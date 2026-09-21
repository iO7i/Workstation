# Dependency audit scope

The direct manifest and usage/features were inspected. A genuine resolved dependency graph does not exist because Cargo is unavailable here. This is **not a clean `cargo audit` result** and is not a legal/SBOM certification.

## Corrected direct pin

`time = 0.3.44` is inside the affected range of **RUSTSEC-2026-0009**. The pin is now `=0.3.47`, the advisory patch floor, preserving formatting/parsing features. The vulnerability concerns RFC2822 parsing; the inspected Workstation paths use RFC3339. No exploitable Workstation path was demonstrated. This is removal of a known affected dependency pin, not evidence that Workstation was attacked.

## Unresolved transitive concern

**RUSTSEC-2026-0285** applies to rustls >=0.23.13,<0.23.45. Workstation's reqwest features select rustls, but no lockfile or resolved build is available. The actual version and exposure remain **unknown**. After approval of the native phase, resolve the graph, run the advisory audit against that real lock and review licenses. Do not fabricate a version or claim a patch has been linked.

## Preserved constraints

No new runtime dependency, service, interpreter or network collector was added. SQLite's existing patched-version guard was not weakened. Python audit utilities are development-only. The existing build/CI `CertificationAuthorized` gate remains intact; this delivery does not trigger it.

Primary sources and reviewed uses are recorded in `sources.json` and exact manifest entries in `dependency-review.json`.
