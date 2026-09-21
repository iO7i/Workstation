# Fixture provenance

`incidents/` contains six manually authored **synthetic inputs**, representing three failure families described in the user's September 18 conversation:

- Docker's stale/current host-runtime socket errors;
- Codex attachment repair at an unverified/wrong registry path versus a filesystem-only repair needing UI verification;
- old worktrees whose age does not prove inactivity or disposable contents.

They are not collected OS snapshots, actual corrupted socket objects, real private chat databases, or Windows reproductions. Replaying them checks policy wording only; it does not establish a vendor root cause or fix anything.

`examples/` contains illustrative JSON contract examples, clearly marked synthetic. The 500 MiB size, PID 1234, and example paths are not measurements of the user's machine. No real credentials or transcripts are included.
