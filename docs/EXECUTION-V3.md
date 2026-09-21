# Explicit effects and safety boundary

## Operator intent versus actual authorization

New registrations and descriptive manifest imports have a canonical typed-input preview
digest. Legacy `record` operations retain their original raw-input digest. Typed effects
use the immutable **stored plan** digest. These are not interchangeable.

A future effect apply requires an exact digest and `--acknowledge-uncertified-execution`.
Plans normally last 600 seconds (absolute maximum 900). Profile/task revisions are immutable;
register a new ID rather than overwriting executable identity. The inherited path/config
values are fingerprinted, and continuation also binds decisions, resource confirmations,
focus and workspace/Work Item state. No credential values enter the plan digest record.

This is consent accounting inside a same-user CLI, **not proof that a human supplied it**.
An unrestricted agent running as that user could call the CLI. DPAPI and an approval flag
must not be marketed as isolation against same-user software.

## Allowed effect variants

- Pinned local integration query: version, Codex thread metadata/quota, local Docker API.
- Registered credentialed task: fixed executable, arguments, cwd identity and script pins.
- Registered continuation: Codex, Claude, Cursor ACP or Grok ACP, exact checkpoint/context.
- OpenAI/Anthropic organization billing read: fixed provider destination and approved key ref.
- Approved public HTTPS endpoint HEAD: fixed resource identity, public-address validation.
- Exact stopped-state runtime/invalid-registry quarantine, or matching-receipt undo.
- Separate pinned Docker Desktop GUI start and local API observation.

No generic shell command, arbitrary HTTP authentication destination, process-kill request,
whole-agent reset, volume deletion or WSL shutdown is accepted.

## Execution and interruption

An OS-held `effects.lock` serializes owned external effects. Each plan permits one execution
record. The journal records intent before mutation. Failures become indeterminate when
external effects may have occurred; no automatic replay occurs. Approved tasks may mutate
remote or local systems and cannot in general be rolled back. Process/session work is not
restored merely because a metadata backup exists.

The CLI exposes history but not a magic recovery of interrupted external actions. Human
review and a new correctly scoped plan are required. Preserve uncertain ownership when a
continuation did not finish cleanly. A successful transport turn is not a passed test or a
completed Work Item.

## Owned process lifetime

Ephemeral workers, provider CLIs and bounded continuation interactions use a Windows
suspended spawn, a private kill-on-close Job Object, then resume. Unix test infrastructure
uses an owned process group. Captured output is bounded; task/provider stderr is not echoed.
The native APIs and teardown failure paths still need real execution tests.

Docker Desktop GUI start is a deliberate exception: a requested GUI must survive the
short launcher. It is started separately, its PID is journaled only as an observation,
and it is not killed when the readiness probe fails. No unrelated instance is stopped.

## Repairs are narrow workarounds

Codex: exact effective-home path, fresh selected error, no running related app/helper,
regular bounded registry with invalid JSON, same-directory quarantine. Payload UUID folders
are never traversed or renamed. Missing or valid-but-problematic registries are not reset.

Docker: exact known user runtime root, current matching error, stopped Docker processes,
a small directory containing only recognized zero-byte runtime socket entries. Reparse
tags are allowlisted only for the socket case. The entire known runtime directory is
renamed in place; no cross-volume copy of broken socket entries is attempted. D:\Docker\Data
is not a target. Startup is a separate effect. Quarantine success means a state transition,
not proof of vendor bug resolution. Root cause remains unresolved unless later evidence says otherwise.

Undo checks the completed receipt, sibling target, identity/digest and that the old source
has not reappeared. It never overwrites a fresh runtime or attachment registry.

## Credentials

Provider bootstrap credentials are local DPAPI references, not recursively resolved through
another provider. Doppler/1Password references resolve only within the selected Workstation
project/environment and after explicit resource confirmation/freshness checks. One provider
profile must be unambiguous. Only requested values reach the task child. Program output is
discarded, not run through an unreliable “secret redactor.”

Memory wiping covers selected byte buffers, not every OS/library/String copy. An authorized
child, its imports/dependencies, or same-user malicious code can observe/exfiltrate values.
Script pinning is not a full dependency lock or sandbox. General secret rotation, portable
backup and external-provider administration are outside this implementation.
