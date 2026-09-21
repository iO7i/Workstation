# Changelog

All notable changes are documented here. This project uses semantic versioning with prerelease identifiers while the public API is still evolving.

## 0.5.0-alpha.1 — 2026-09-21

### Added

- Durable external-run state with schema-v5 SQLite persistence and append-only events.
- Revision-checked cancellation, local recovery, exact provider reconciliation, and bounded cached watches.
- Independent handshake, session, idle, absolute, cancellation-grace, and reconciliation deadlines.
- Content-hash baselines and approval-bound verification contracts with exact allowed and forbidden paths.
- Offline fake-provider end-to-end coverage for completion, crashes, cancellation, deadlines, reconciliation, and verification.

### Changed

- Database upgrades now reach schema 5 and create a verified backup before the v4→v5 migration.
- Positive verification retains Work Item state and primary ownership for explicit operator action.
- CI and the Windows build gate exercise all features and deny Clippy warnings.

### Security

- Durable runs pin integration, executable, work version, checkpoint, workspace digest, session, and turn identity.
- Cancellation cannot target an arbitrary PID, and a committed cancellation wins a concurrent timeout observation.

### Known limitations

- The release is unsigned and live-provider execution is not generally certified.
- Durable history has fixed storage budgets and no compaction in this alpha.
- See `docs/DURABLE-RUNTIME-LIMITATIONS.md` for the complete boundary.
