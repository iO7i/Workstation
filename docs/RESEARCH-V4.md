# Primary-source decisions — v0.4 implementation-only

Reviewed 18 September 2026. URLs and purpose are recorded in SOURCES-V4.json.
The sources inform implementation. They do not certify the resulting code, authenticate
an account, establish a reviewed immutable upstream revision, or prove compatibility.
No full vendor source, credentials or transcripts are copied into this delivery.

## Decisions that changed the code

- **Copilot:** use an explicitly selected protocol revision and Content-Length JSON-RPC,
  not ACP line framing. Use account quota and model price operations separately. Retain
  unknown/unlimited quota states. Default-deny unsupported callbacks; reassert per-session
  permissions on resume. Several generated methods are experimental: a changed shape
  fails rather than silently bypassing policy. This is a narrow native wire adapter,
  not a bundled official SDK or all-VS-Code integration.
- **Claude:** configured statusline metadata can include five-hour/seven-day rate limits
  and reset timestamps. Absence stays unknown. Reported cost remains an estimate rather
  than an invoice; no independent consumer quota endpoint or cookie scraping is invented.
- **Gemini/OpenCode/Cline:** use documented ACP launches and advertised load/list/mode/auth
  features. No claimed parity. Gemini session token totals are not current-context usage
  or subscription capacity. Roo/Windsurf remain packet-only in this version.
- **Worktrunk:** use scoped local schema2 listing and exact approved creation. Hooks and
  project custom commands are not loaded as instructions. Worktrunk's broader background
  removal/branch lifecycle is not substituted for the tool's conservative native Git path.
- **Git:** registered worktree inventory is combined with complete content-state and
  external-writer coordination. Preserve the exact HEAD under an explicit new ref; removal
  never uses force, branch deletion or broad prune. Acknowledgement is not an OS-exclusive
  lock: concurrent unmanaged changes remain a stated risk and require later native tests.
- **Ownership:** observed PID plus birth and host epoch is different from a name match.
  Configured hook ancestry is nonexclusive. Unknown/shared/denied observations protect
  candidates. No absent-parent or idle-process rule authorizes termination.
- **Secrets:** implement immutable DPAPI versions and an atomic metadata head transition.
  Local revocation blocks Workstation resolution but does not revoke the external token.
  Metadata restore cannot magically make another machine decrypt the old user-bound value.
- **Archives:** a verified file is written before journal rows become archive locators.
  Backups include packs referenced by that exact backup database. Integrity receipts are
  SHA256 checks, not signatures; neither archive nor WAL cleanup guarantees OS disk shrink.

## Verification deliberately deferred

No native vendor version was run, no transport exchange captured, and no authenticated
provider was called. Read-only scans and cached queries are distinct: cached context must
not trigger searches, quota requests or new vendor launches. The source remains a candidate
until compilation, permissions, crash/recovery, privacy and real task continuation are tested.
