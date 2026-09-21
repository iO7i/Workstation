# Acceptance matrix — FS-1.0

These are **required implementation tests**, not tests already executed in this design session. See `VALIDATION.md` for the much narrower artifact checks actually run.

Destructive/credential tests use disposable Windows VMs, fake providers, synthetic credentials, and fixture repositories. Never introduce faults into the user's active projects. The implementation must publish exact builds, privileges, fixtures, outcomes, and known limitations.

| ID | Release | Fixture / trigger | Required result | Test environment |
|---|---|---|---|---|
| BASE-01 | R0 | Clean supported Windows VM; no Node/Python/Docker installed | Core starts without those dependencies; registered scope only. | Windows integration |
| BASE-02 | R0 | User-selected data drive absent at startup | Explicit unavailable-root error; no silent C: state creation. | Windows integration |
| BASE-03 | R0 | Low disk, denied directory, scan deadline exhausted | Partial/denied/timed_out coverage, not empty or GREEN. | Unit + Windows |
| BASE-04 | R0 | Two simultaneous scans and one long report reader | Bounded writer contention, no indefinite transaction or unbounded retries. | Database + Windows |
| BASE-05 | R0 | SQLite runtime lacks required fix | Startup/release qualification fails before WAL use. | Build + Windows |
| PROC-01 | R1 | Confirmed active agent with several legitimate child servers | Protected associations; no orphan conclusion from multiplicity. | Synthetic + live Windows |
| PROC-02 | R1 | Old idle shared daemon whose parent exited | Suspected/unknown ownership; no stop plan. | Synthetic + live Windows |
| PROC-03 | R1 | PID reused after diagnosis by an unrelated program | Birth identity mismatch invalidates plan; replacement never signalled. | Windows adversarial |
| PROC-04 | R1 | Two sessions share one MCP instance | Many-to-many consumers preserved; ending one session cannot authorize kill. | Synthetic + live Windows |
| PROC-05 | R1 | Agent window closes but task/terminal continues | No inference that task is complete solely from window exit. | Windows integration |
| PROC-06 | R1 | Process command line contains a credential | Keep allowlisted features only; raw command line absent from persistence/reports. | Synthetic + Windows |
| PROC-07 | R1 | CPU/working-set/commit counters with shared pages | Units and memory concepts distinguished; no unique-RAM claim from shared sum. | Collector tests |
| FS-01 | R1 | Overlapping roots and hardlinks | Deduplicate where identity available; otherwise label totals as upper bounds. | Windows filesystem |
| FS-02 | R1 | Junction escapes approved root | Do not traverse; record link and blocked/partial coverage. | Windows filesystem |
| FS-03 | R2 | Reparse target changes after plan approval | Live file/volume identity mismatch blocks apply. | Windows adversarial |
| FS-04 | R1 | Cloud placeholder and WSL/network UNC path | No hydration or unapproved remote traversal in normal scan. | Windows filesystem |
| FS-05 | R1 | Sparse Docker VHDX large logical length | Logical/allocated measurements separated; unknown allocation stays unknown. | Windows filesystem |
| GIT-01 | R1 | Merged clean-looking worktree containing ignored .env | Worktree protected; neither cache nor merged state implies disposable. | Git fixture |
| GIT-02 | R1 | Detached HEAD with local-only commits | Record and protect the exact OID before any retirement plan. | Git fixture |
| GIT-03 | R1 | Stale remote refs, untracked files, submodules or rebase | No automatic fetch or cleanup; clear blockers. | Git fixture |
| GIT-04 | R1 | No PR and expired agent lease | No removal permission; lease is only one evidence source. | Unit + Git |
| GIT-05 | R2 | Worktrunk remove may delete branch | Do not substitute for a narrower planned non-force worktree removal. | Adapter fixture |
| GIT-06 | R1 | Malicious repository fsmonitor/hook/alias configuration | Read-only probe does not execute project-controlled helper. | Isolated adversarial Git |
| ACT-01 | R2 | Approval digest/policy/target changed | Invalidate consent and require a new reviewed plan. | Unit + integration |
| ACT-02 | R2 | Interruption immediately before/after each filesystem step | Reconcile journal vs actual state; never blindly replay unknown completion. | Fault injection |
| ACT-03 | R2 | Probe hangs or floods stdout and stderr | Hard deadline/output caps; drain both; only owned subprocesses affected. | Windows subprocess |
| ACT-04 | R2 | Two repair invocations race | One action lock; second invocation blocked or bounded wait. | Windows integration |
| ACT-05 | R2 | Unknown fingerprint predicate or operation ID | Rule disabled; never interpreted as a shell program. | Contract + domain |
| ACT-06 | R2 | Process stop succeeds; user requests rollback | Report irreversible memory/task interruption; do not claim restoration. | UX + action tests |
| DOCKER-01 | R2 | DOCKER_HOST/context points to remote healthy engine | Do not certify local health and do not act on remote engine. | Adapter + isolated endpoints |
| DOCKER-02 | R2 | Old backend.error.json with currently healthy engine | No runtime repair. | Adapter fixture |
| DOCKER-03 | R2 | Fresh exact AF_UNIX error and verified inactive runtime | Same-volume narrow quarantine; data VHDXs untouched; finite verification. | Windows disposable VM |
| DOCKER-04 | R2 | Another active WSL distro; Docker engine not inspectable | No global shutdown; explicit interruption decision, not assumption of no work. | Windows integration |
| CODEX-01 | R2 | Current Codex conversation runs the repair request | Repair blocked until app offline; current agent is not terminated. | Windows integration |
| CODEX-02 | R2 | Unknown home/registry schema or merely missing registry | No hard-coded reset; explain unsupported/unknown. | Adapter fixture |
| CODEX-03 | R2 | Confirmed corrupt offline registry with existing payloads | Back up registry only; payload paths and contents preserved; UI paste separate test. | Windows + manual UI |
| CHR-01 | R4 | Imported transcript says an earlier decision is superseded | Create proposal; no accepted decision replaced. | Domain tests |
| CHR-02 | R4 | Two proposals concurrently replace same accepted head | One serialized acceptance or explicit conflict; no lost update. | Database + domain |
| CHR-03 | R4 | Accepted replacement effective next week | Today returns old applicable decision; next week returns new, with sources. | Temporal domain |
| CHR-04 | R4 | Retrospective import recorded later than effective date | As-known and as-effective queries return appropriately distinct results. | Temporal domain |
| CHR-05 | R4 | Decision cycle or overlapping scope without explicit override | Reject cycle or flag scope conflict; never latest-timestamp-wins silently. | Domain tests |
| CHR-06 | R4 | Source deleted or inaccessible | Retain accepted text with unavailable-source flag; no invented evidence. | Domain tests |
| ATLAS-01 | R3 | Same key name in dev and production | Resolve only exact project/environment; no fallback. | Provider fixture |
| ATLAS-02 | R3 | Manifest includes commands, grants or credentials | Reject shape and/or content; descriptive import grants no authority. | Contract + security |
| ATLAS-03 | R3 | Token URL, link-local metadata URL, malicious redirect | Block or require explicit registered private target; never implicit credentials. | Isolated HTTP tests |
| ATLAS-04 | R3 | HTTP 200 from wrong provider resource | Connectivity success remains distinct from resource-identity verification. | Provider fixture |
| SECRET-01 | R3 | Local DPAPI same user decrypt and other identity attempt | Document genuine decrypt behavior on tested Windows setup; no portable backup claim. | Windows identity test |
| SECRET-02 | R3 | Secret passed to approved test program | No value in argv, parent globals, logs, database, MCP or normal reports. | Windows integration |
| SECRET-03 | R3 | Child prints/encodes/sends credential | Demonstrate trust-boundary limitation; no guarantee injection prevents exfiltration. | Isolated adversarial test |
| SECRET-04 | R3 | Provider bootstrap token and unrelated secrets in parent env | Child env does not inherit unnecessary provider tokens/other credentials. | Windows integration |
| SECRET-05 | R3 | Task executable/script/working-directory changed after approval | Approval invalidated; local task hash and context rechecked. | Windows adversarial |
| SECRET-06 | R3 | Crash between encrypted file write and metadata commit | No plaintext; reconcile encrypted orphan version; prior version remains valid. | Fault injection |
| SECRET-07 | R3 | Ciphertext copied to fresh machine/profile | Restore explains decryptability limitation; metadata restore not vault certification. | Windows migration |
| MCP-01 | R5 | Agent asks for arbitrary file/SQL/command/secret value | No such tool available; project-limited metadata only. | Protocol tests |
| MCP-02 | R5 | Injected instructions in source excerpt | Treat as untrusted data; no grant or repair capability. | Adversarial import |
| UI-01 | R5 | HTML markup, Arabic text, bidi controls in filename/decision | Escape HTML; expose misleading controls; preserve Unicode source separately. | Renderer tests |
| INT-01 | R4 | Claude WorktreeCreate requested only for observation | Do not register replacing hook; leave existing behavior unchanged. | Adapter + manual |
| INT-02 | R4 | User modifies integration after installation | Uninstall uses receipt/diff; does not overwrite later edits. | Integration test |
| OPS-01 | R5 | Upgrade fails migration or backup disk is full | Preserve original; abort; documented compatible restore. | Windows upgrade VM |
| OPS-02 | R5 | Own quota exhausted while active repair backup protected | Stop optional growth; do not silently delete protected backups or user data. | Retention tests |
| OPS-03 | R5 | Existing Docker startup task present | Audit/reuse or preserve; no competing startup repair loop. | Windows integration |
| OPS-04 | R5 | Cold cached context and 20repo/200worktree/500process workload | Measure targets; publish hardware, scope, latency and coverage, not promises. | Performance Windows |

## Release evidence

For every executable test record test ID, tool/build/adapter versions, OS build, fixture digest, timestamps, environment privileges, observed result, expected result, changed targets, evidence files, and limitations. “Passed in a fixture” is not “all affected vendor versions supported.”

Minimum negative gate: no protected fixture data is changed, no unrelated process is terminated, no live worktree is removed, no credential appears in generated evidence, and no unknown/partial evidence becomes an automatic permission. A single violation blocks the release. This is a finite-test gate, not proof of universal safety.

Read-only release gates may pass while mutation features remain explicitly disabled. Inventory-only adapters are not failures if they accurately advertise their limits.

## Accepted D1 additions (R4/R5)

[FS-1.0-D1-ACCEPTANCE.md](FS-1.0-D1-ACCEPTANCE.md) adds **28 required product cases**.
They are not extra passed tests. The base 62-case matrix remains intact. Record reference/schema
checks separately from native implementation, integration and certification evidence.
