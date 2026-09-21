# Recovering Workstation's own first-slice metadata

This document does not repair Codex, Claude, Cursor, Docker or WSL.

## Missing home

Reconnect/restore the intended local drive and pass its original `--home`. Do not initialize a substitute home on C: to hide the error. There is no automatic fallback or profile relocation.

## Interrupted initialization

A new partially initialized home may remain. Inspect it. `init` intentionally refuses to overwrite any existing directory. Use a different explicitly selected empty home rather than deleting unknown files. Existing product/application directories must never be used as an initialization target.

## Backup

`workstation --home ... backup` uses the own SQLite backup API, an integrity check and a SHA-256 receipt. Both `.sqlite` and `.receipt.json` are required to treat a backup as complete. A backup without a receipt after interruption is unverified, not a successful recovery artifact. At most five completed pairs fit the initial budget; inspect and archive older copies manually rather than silently deleting them.

## Manual restore in this source slice

A restore CLI is deferred. Before any operator restore, exit Workstation commands and preserve the entire original home, including any own WAL/SHM files. Verify the backup receipt/hash and SQLite integrity in a separate local directory. Restore only Workstation metadata into a separate known home with a compatible config/schema; do not overwrite a live database or treat a copied live `.db` without WAL as a valid backup. The own config's home identity is checked, so changing locations requires deliberate configuration review rather than blindly copying it.

No DPAPI vault is present in R0. Future vault recovery will be a separate identity/decryption concern, not inferred from a valid SQLite backup.
