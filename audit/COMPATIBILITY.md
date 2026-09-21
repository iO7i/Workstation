# Compatibility and migration notes — audit candidate

- Candidate version: **0.4.0-alpha.2**. Database remains **schema 4**; no data migration or schema downgrade is needed merely to adopt the source fixes.
- **Effect policy changed to `workstation.effects.v4.audit1`. Existing saved effect approvals must be replanned and reviewed.** Never edit an old digest or payload in place. New archive operations include the digest of the exact selected records.
- Workspaces using `assume-unchanged`, `skip-worktree`, sparse-checkout or unreadable Git state now remain blocked for destructive operations. This is intentional. Do not auto-clear these flags as a workaround; inspect and preserve the user's changes first.
- Plan-fit cycle imports add `account_alias`. Older data remains parseable but produces `unknown` rather than silently combining accounts. The updated example labels its account synthetic.
- Ciphertext read/write ceiling is 128 KiB, with the same 16 KiB plaintext ceiling and existing DPAPI bindings. No encryption format or user secret is migrated by this audit.
- A `restore.pending` marker protects interrupted target homes. Do not blindly delete the marker. Check the exact backup and archives, or retry to another new home after recovering preserved data.
- Optional context failures appear in `optional_section_failures`. Essential decisions/resources still fail closed. No missing optional data is claimed to be empty/healthy.
- time direct pin changes to 0.3.47. Real lockfile resolution, full advisory/license review, rustfmt, compilation and Windows/provider testing remain deferred pending the user's green light.
- Current synthetic fixtures and static checks are not compatible vendor certification. No Windows device was used in this audit.
