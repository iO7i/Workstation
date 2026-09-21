# Recovery source and restore semantics — current revision

The previous source contained metadata-only repair plans. They remain historical records.
The new `effect` engine has concrete, narrowly scoped quarantine/undo/start implementations;
none was executed or certified for this delivery. See EXECUTION-V3.md for exact restrictions.

Quarantine is not deletion and not a root-cause fix. Restoring a backup cannot undo remote
API changes, unsaved process state or an agent's modifications. Receipts preserve uncertainty
when outcomes are not known. One execution per plan prevents accidental replay.

Schema upgrade makes a verified metadata backup before each migration. A schema1 upgrade
can reach2 then encounter a3 failure; the2 state remains valid and the outcome must be reported,
not called an all-or-nothing1→3 rollback. Schema3 accepts the original project/work/history
records without resetting the database. Restore is only into a new explicit home with receipt
checks; no overwrite of the live home. Native migration/restore behavior remains unexecuted.

Local DPAPI ciphertext is separate from metadata backups. Copying it across Windows users
or devices is not a supported portable-vault recovery procedure. General secret rotation,
external provider backups and disaster-recovery tooling are not implemented.

The owned DB budget still limits growth. Exhaustion refuses optional writes instead of
silently pruning indispensable approvals/plans/recovery evidence. Long-term archive/retention
UX for immutable effect journals remains future implementation work.
