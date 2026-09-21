# v4 implementation notes

This extends schema3 and the exact 0.3 source. Existing source/evidence is preserved in history.

## Observation is not authority

Health scans run bounded local metadata workers. Profile/home choices are explicit; vendor GUI
overrides are not silently guessed. The process graph captures PID plus process creation time
and a conservative uptime epoch. PID reuse changes identity; discontinuous clocks/uptime expire
bindings. Disk executable digests are not signatures or proof of loaded image contents.

Managed continuation records the process birth and protocol session before prompting. Optional
configured start hooks can correlate the actual caller's observed ancestry with a registered
binary. Payload PIDs are not accepted. Hook bindings are short-lived, shared/protected and never
stale-process disposal authority. Missing/inaccessible ancestry remains unknown.

Collision analysis combines Work Item/workspace/repository identity, branch and relative changed
paths. Stale leases increase uncertainty rather than removing actors. Existing source cannot
know future edits or prove no unmanaged writers. Every finding is advisory.

## Typed workspace lifecycle

`effect prepare` builds immutable review material. Removal requires fresh complete observations,
secondary unlocked checkout, no dirty/untracked/ignored/operation blockers, no unfinished/uncertain
known work, no manual protection, and explicit external-writer coordination. The plan pins Git
binary, directory/repository identity, exact state and preservation ref. Apply checks again,
creates a new ref preserving HEAD, checks content/state again, then runs Git removal **without
force**, followed by an absence check that does not confuse access errors with nonexistence.
No branch deletion or repository-wide prune. The preservation ref is for committed HEAD, not
magic recovery of unsaved edits. A same-user race outside the coordination agreement remains possible.

Creation binds source checkout, base commit, Git/profile digests and destination-parent identity.
Configured checkout filters and unsupported submodule policy block creation. Git/global hooks are
disabled. Optional Worktrunk uses an owned empty config, exact path template, no hooks/no-cd, no
copy-ignored/execute/clobber/remote PR lookup. Postconditions inspect actual Git registration.
Partial create failures are recorded and preserved, never cleaned automatically.

## Secret versions

Local DPAPI binding contains project/environment/resource/version and bytes. A new immutable
ciphertext is flushed before a database transaction switches the head via compare-and-swap.
A crash can leave unreferenced encrypted bytes, not destroy the old active value. Generation digests
invalidate outstanding plans. Local revoke blocks all further local resolution, including fallback
to legacy ciphertext. Old ciphertext is retained; remote tokens and encrypted backups are not
magically revoked or securely erased. Recovery reports metadata separately from decryptability.
User/machine changes may require reentry or provider reauthentication; no portable DPAPI export.

## Journals and storage

Only old terminal succeeded/failed/cancelled effect payloads are archived. Running and indeterminate
ones stay online. Checksummed JSON packs have 8MiB each, 64MiB total and bounded counts. SQL stores
stable IDs/digests and archive locators. Readers verify archive and entry checksums before hydration.
Backup copies packs referenced by the snapshot database, not a later live listing. Restore requires
those exact local companions and a NEW home. Checksums are not signatures against same-user tampering.
Authoritative history is not silently deleted, and reusable SQLite pages do not imply OS file shrink.

## Transports

Copilot uses Content-Length framing, not ACP newline JSON. The client pins protocol3, limits
frames/output/time and denies unsupported callback requests. Session permissions/options are
reasserted on resume; user-message correlation precedes idle completion. Native SDK behavior still
needs verification. ACP honors advertised list/resume/load/close features and explicit noninteractive
auth method selection. The user must separately authorize any prompt, billing request or quota query.

Session lists are filtered to the registered repository identity and do not imply live activity.
Configured hook telemetry remains agent-reported; directly received values are observations, not
proof of authenticated account identity. Account/profile cache isolation prevents cross-account
replacement. No browser cookies, auth-page scraping or guessed private endpoints.

## Discovery and Chronicle

Availability is a cached exact profile observation, not compatibility certification. Adoption creates
no records by itself: it produces separate reviewable Atlas and Chronicle proposal drafts. Accepting
that decision is another explicit operation. Source revision changes require renewed review.
Hook state is metadata, not approved reasoning. Lifecycle activity is coalesced per session; explicit
boundaries remain recorded. MCP remains read-only and never triggers live collection from context.

## Known nonclaims

No certification, comprehensive handle ownership, universal vendor-health parity, same-user sandbox,
portable secret backup, remote credential administration, Roo/Windsurf native transport, or unlicensed
benchmark feed. These limits are real scope/implementation boundaries, not synthetic test passes.
