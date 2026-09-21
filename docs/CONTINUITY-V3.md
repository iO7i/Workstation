# Continuation source contract

The work belongs to Workstation; proprietary sessions remain opaque vendor identifiers.
The current native transport implementations are Codex App Server, Claude noninteractive
print/JSON, Cursor ACP and Grok ACP. Other adapters retain packet-only fallback.

Preparation binds the integration revision, Work Item version, checkpoint, workspace
identity/HEAD/branch/status metadata, applicable decisions, resource confirmations and focus.
Application rechecks these before reserving a target primary. No existing unreleased primary
is silently stolen, even after its lease expires. A possibly active worker sharing the
workspace blocks the new reservation. Reservations are advisory, not OS file locks.

A new vendor session identifier is bound to the roster **before** its prompt starts.
Resume requires an explicitly known compatible ended session in the same workspace.
A clean turn ends this controlled interaction and releases its assignment; an uncertain
result preserves protective ownership. No implicit done state or unreported ownership transfer.

## Vendor transports

- Codex: initialize/initialized, thread/start or thread/resume, turn/start, matching completion.
  Uses readOnly or bounded workspaceWrite requests; no approval-bypass shell mode.
- Claude: print JSON, explicit session UUID or known resume ID, bounded turns, plan or
  acceptEdits permission mode. Structured outcome only; transcript output not retained.
- Cursor: `agent acp`. Grok: `grok --no-auto-update agent stdio`.
  ACP protocol negotiation, supported auth method, session/new or advertised session/load,
  optional advertised plan mode, prompt and scoped permission replies.
- Unsupported callback or capability: deny/error or packet-only, never fake successful parity.

ACP allow-once applies only to declared read/edit/search requests with explicit paths within
the approved worktree. Unknown paths, escape, sensitive configuration, execute/fetch/delete
and other kinds are denied. This is not OS confinement of a vendor that performs its own
operations; verify actual agent behavior later. Some tasks may stall or fail under these
limits. Loosening permission policy needs an explicit reviewed change, not an automatic retry.

## Lifecycle ingestion

`lifecycle import` is an explicit reviewed input. `lifecycle hook` receives bounded stdin
only through an explicitly registered matching profile and `--accept-agent-report`.
Supported projection reads only selected operational fields. It does not follow transcript
paths or read full prompts/responses. Claude Stop means turn end, not SessionEnd. Events
remain `agent_reported`; configured hooks do not authenticate the native process owner.
Repeated source IDs with identical data are idempotent; changed data with the same ID is rejected.
Lease renewal names the exact assignment and session, with a bounded duration.

## Freshness limitation

The inherited workspace fingerprint is metadata-based. A same-size edit with a restored
mtime can evade it. A continuation packet explicitly requires the receiver to inspect
current content and tests. Neither a hash of this metadata nor an RPC completed-turn marker
proves semantic correctness. Full observed multi-agent execution is deferred, not simulated
as a success in this package.
