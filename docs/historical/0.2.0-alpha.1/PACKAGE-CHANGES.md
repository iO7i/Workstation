# Package revision d1

## Changed

- Accepted FS-1.0-D1: Contextual Resource Discovery inside Capabilities.
- Linked it in source README, architecture, slice decision and builder handoff.
- Included the complete amended frozen design under docs/frozen-design.
- Added six strict draft authoring contracts, synthetic examples and a development-only
  reference policy under specs/discovery/v1; added 28 R4/R5 product acceptance requirements.
- Added scripts/validate_discovery.py, actual specification test results and baseline checksums.
- Refreshed package checksums and kept old native-build limitations explicit.

## Not changed

- Rust source/test code, three-crate layout, Cargo manifests/toolchain declarations, native
  build/CI scripts, R0 input/output contracts, applied schema and R0 fixtures.
- User-visible runtime features or command behavior.
- The absence of a compiled Windows executable and real Cargo.lock in this source candidate.

## Builder instruction

Read docs/NEXT-BUILDER.md, docs/scope/FS-1.0-D1.md and validation/D1-VALIDATION.md.
Complete native R0 validation first. Carry the accepted D1 requirements into R4; do not
advertise synthetic fixtures or specification tests as a reviewed catalog or shipped feature.
