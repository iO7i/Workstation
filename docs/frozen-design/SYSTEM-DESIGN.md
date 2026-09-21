# Workstation — Frozen System Design v1.0

**Scope lock:** FS-1.0 + accepted FS-1.0-D1 (Contextual Resource Discovery; R4/R5)  
**Design date:** 18 September 2026  
**Status:** Frozen design specification; implementation and Windows certification are not claimed.  
**Working product/CLI name:** Workstation / `workstation`. This is an internal identifier, not a trademark-availability assertion.

## 0. Read this first

Build one local, project-centered application that helps an AI-heavy developer answer five questions:

1. **Health:** What is unhealthy on this computer, and what is the smallest justified recovery?
2. **Workspaces:** What belongs to each project, worktree, session, and process, and what must be protected?
3. **Chronicle:** What happened, what was decided, why did it change, and which decision applies now?
4. **Resources:** Where are the correct repository, environment, production URL, service, runbook, and credential reference?
5. **Capabilities:** Which built-in action or established external tool solves the problem already detected?

The architecture is **one Rust application, one local SQLite metadata database, encrypted local secret blobs, and versioned adapters**. Its ownership graph is ordinary relational data with evidence-bearing relationships. It is not a graph database, microservice system, cloud control plane, autonomous repair agent, or new coding agent.

The v1 product is Windows 11 x64, local-first, free to use locally, and designed for an open-source release. No account, subscription, Docker, WSL, Node, Python, hosted database, LLM API, or continuous service is required to run the core. Optional integrations have their own installation, licensing, authentication, and connectivity requirements. Prices and free-tier quotas are not hard-coded product assumptions.

The product does not replace the user's development orchestrator, deployment pipeline, Git hosting, secrets provider, or agent. Existing commands such as `vertexctl dev seo` remain owned by their existing project tooling; Workstation may explain or register them but must not become a competing orchestrator.

**Evidence convention:** `[Sxx]` references a primary source in the source appendix and `sources.json`. Requirements, thresholds, API contracts, and release gates below are design decisions, not claims that a vendor already implements them. Earlier discussion and public bug reports are research leads; no repair rule ships solely because an earlier answer described it as safe.

---

## 1. Frozen scope and exclusions

### 1.1 Committed end-state for version 1.0

| Module | Included in 1.0 | Explicit boundary |
|---|---|---|
| Health | Agent/runtime inventory; bounded resource observations; known-failure detection; provenance; approved recovery plans; before/after verification | No generic process killer, system optimizer, registry cleaner, or automatic upgrades |
| Workspaces | Git repositories/worktrees; dirty/untracked/ignored state; local-only work; session associations; protection leases; storage; explicit lifecycle plans | No automatic merging, rebasing, pushing, branch deletion, or autonomous removal of unknown worktrees |
| Chronicle | Structured events, decisions, rationale, evidence, effective dates, supersession, conflict detection, current-state summaries, approved imports | No automatic assertion that inferred decisions are authoritative; no blanket ingestion of all chats |
| Resource Atlas | Project/environment/service catalog; URLs; provider identifiers; runbooks; explicit verification; secret references | No cloud provisioning, deployment, password-manager clone, or inferred production permission |
| Secrets | User-scoped Windows DPAPI backend; reference-based provider interface; explicit credentialed task execution; metadata-only discovery | No protection claim against malicious software with the same Windows identity; no agent-facing reveal API |
| Capabilities | FS-1.0-D1: bounded project-contextual discovery of tools, skills, workflows, docs, references, datasets and benchmarks; reviewed local catalog; scoped feedback; optional reviewed research handoff | No sixth module, marketplace, synchronous context-path search, silent installs, agent-facing mutations, or arbitrary downloaded scripts |
| Access surfaces | Human CLI; versioned JSON; static local HTML/Markdown reports; optional project-scoped read-only MCP | No authenticated web service, Electron/Tauri shell, public port, or mobile application |
| Maintenance | Own-data retention, backup/export/restore, rollback-aware upgrades, optional read-only scheduled check | No automatic user-data cleanup or continuous high-frequency agent watchdog |

### 1.2 Adapter commitments

**First-class diagnostic adapters in 1.0:** Codex, Claude Code/Desktop, Cursor, VS Code/Copilot. These are separate product variants inside adapters, not one assumed filesystem layout. Each operation advertises its tested version and coverage. A named adapter does not imply access to every proprietary session or process.

**Inventory/discovery-only initially:** Gemini CLI, Grok tooling, Cline/Roo, Windsurf, OpenCode, and other recognizable runtimes. Generic process/storage evidence can still be shown, but version-specific repair and private-state parsing require separately certified adapters. Expanding this matrix is an adapter release, not a new platform subsystem.

**Secrets in 1.0:** complete local DPAPI backend; Doppler and 1Password execution adapters where documented, tested CLI capabilities exist. Infisical and Bitwarden references, discovery, and integration guidance are included; execution adapters follow the same contract and remain explicitly `reference_only` until certified. No placeholder provider is advertised as a working vault.

**Specialized-tool integration:** Git is the foundational workspace backend. Worktrunk is optional advanced workflow integration, Entire is an optional provenance/import source, and Codbash is optional session navigation. The core remains useful without them. Their current projects document these distinct roles. [S13][S14][S15]

### 1.3 Outside version 1.0

Cloud sync; enterprise fleet management; billing; organization RBAC; remote remediation; telemetry collection service; a kernel driver; ETW continuous tracing; attaching arbitrary existing agents to Job Objects; an agent orchestrator; full transcript replay; a vector database; semantic graph infrastructure; blanket credential scanning; a cloud secrets vault; autonomous production tasks; automatic Git cleanup; cross-OS parity.

These are excluded, not hidden implementation TODOs. Additions require an explicit scope amendment, a concrete user need, a maintenance budget, and an acceptance gate.

### 1.4 Safety language

Use `observed`, `suspected`, `verified`, `partial`, `unknown`, `protected`, `blocked`, and `recovered`. Do not equate `old` with disposable, `idle` with abandoned, `process absent` with session complete, or `HTTP 200` with correct production identity.

**Zero unsafe actions in the acceptance suite** is a release requirement, not a mathematical promise of zero future failures. Process termination is not reversible; removing a directory is not reversible unless a complete, restorable backup actually exists.

---

## 2. Architecture decisions

| ID | Frozen decision | Reason |
|---|---|---|
| A01 | Modular monolith, three Rust crates, one shipped executable | Keep installation and maintenance smaller than the problem being solved |
| A02 | Windows 11 x64 native target; MSVC Rust toolchain | Native Windows API access without requiring a scripting runtime; Rust documents first-class Windows MSVC support [S01][S02] |
| A03 | Local SQLite, foreign keys, WAL, brief writes, no network/shared DB path | Relational joins cover the ownership graph; WAL must be local to one host [S03] |
| A04 | Bundle and pin a patched SQLite build; verify linked runtime at startup/test | SQLite documents the WAL-reset corruption fix in 3.51.3 and later, with named backports; do not assume a wrapper version identifies the linked engine [S04] |
| A05 | Normal-user operation; one-shot elevation only for narrowly typed actions | Avoid a permanent privileged service and unnecessary administrator access |
| A06 | Evidence collection, inference, planning, approval, execution, and verification are distinct stages | Prevent a log string or LLM conclusion from becoming authority to mutate a machine |
| A07 | Filesystem identity and process creation time matter; paths/PIDs alone do not | Microsoft documents PID reuse; reparse points alter pathname behavior [S06][S08] |
| A08 | Vendor CLIs/public interfaces first; private formats only behind version gates | Lower maintenance and prevent fabricated cross-vendor parity |
| A09 | Project metadata and approved history are canonical locally; vendor repositories and vaults remain canonical for their own data | No duplicate authoritative deployment or secrets store |
| A10 | DPAPI is optional convenience encryption at rest, not a sandbox against the agent | Same-logon credentials normally permit decryption; process environments can be inherited [S09][S10] |
| A11 | No LLM needed for safety decisions or normal use | The reliability tool must work when models, subscriptions, or network access fail |
| A12 | CLI + JSON + static HTML first; optional read-only stdio MCP | One usable surface for humans and agents without another local web platform |
| A13 | Project manifests may describe; only locally approved policy may authorize | Repositories, transcripts, hooks, and remote text are untrusted input |
| A14 | One plan executor, bounded retries, explicit partial outcomes | Avoid recursive repair loops and duplicate startup paths |
| A15 | No automatic rewriting of third-party databases, package contents, or full agent homes | Preserve histories, active work, authentication, and supported upgrade paths |

**Technology choices:** Rust stable pinned in `rust-toolchain.toml`; `clap` for the CLI; `serde`/`serde_json` for typed contracts; `rusqlite` for SQLite access/backup; focused Microsoft Windows bindings for process/file/DPAPI operations; a maintained secret-buffer/zeroization crate; the maintained MCP Rust SDK only for the optional protocol boundary. Exact dependency versions are chosen once the lockfile is generated, reviewed, and tested, not guessed in this design. Commit `Cargo.lock`, record the SQLite source/version, disable dynamic SQLite extensions, and produce an SBOM. Rust bindings and SQLite APIs are documented in the primary sources. [S02][S05]

A general-purpose async framework is not a requirement for every module. A small bounded worker pool and cancellation deadlines are sufficient for collection; use async only where an adopted SDK needs it.

---

## 3. System shape and dependency direction

```text
Human CLI / JSON / static report / optional read-only MCP
                         |
                 Application use cases
                         |
       +-----------------+--------------------+
       |                 |                    |
    Observe          Plan/approve          Context/query
       |                 |                    |
Windows/Git/agent     Safety policy       Chronicle/Atlas
provider adapters        |                    |
       |          Typed action executor       |
       +-----------------+--------------------+
                         |
        Metadata store + evidence + bounded audit
                         |
     Local SQLite / DPAPI blobs / owned backup files
```

This drawing is logical, not a collection of services. No broker, REST API gateway, message bus, Redis, or graph server is needed.

### 3.1 Three crates

`workstation-core` contains domain models, evidence semantics, pure ownership rules, fingerprint evaluation, plan construction, permission evaluation, decision transitions, context selection, and interfaces for external effects. It must not call the OS or fetch secrets directly.

`workstation-platform` implements Windows collection, filesystem identity, Git commands, adapter parsing, SQLite, provider access, bounded subprocess execution, DPAPI, and typed filesystem/process actions.

`workstation-cli` wires use cases together and supplies terminal output, JSON serialization, report rendering, initialization, maintenance commands, and the optional MCP server. A hidden elevated action entry point may exist in the same binary but must revalidate a typed plan; it is not a generic administrator shell.

Adapters import shared domain types and runner services. They do not invoke other adapters directly, make their own policy decisions, or create background processes outside the runner.

### 3.2 Sources of truth

| Information | Authoritative system | Workstation stores |
|---|---|---|
| Git history and worktree registration | Git repository | Identity, observations, references, approved protection state |
| Vendor session content | Agent/approved export | Metadata and source pointers; excerpts only with opt-in |
| Current process/file state | OS at observation time | Timestamped observations with coverage |
| Approved project decisions | Chronicle approval history | Immutable revisions and derived current view |
| Cloud service identity | User confirmation/provider evidence | Scoped resource records, provenance, verification freshness |
| Secret value | DPAPI backend or selected provider | Opaque references; local backend ciphertext only |
| External capabilities | Curated integration manifest | Version/platform support and locally observed installation |

No diagnosis may silently promote an observation into an approved project decision.

---

## 4. Installation, paths, and coexistence

### 4.1 User installation

Example for this workstation:

```text
D:\Tools\Workstation\
  workstation.exe
  release-manifest.json
  licenses\

D:\Workstation\
  config.json
  workstation.db
  workstation.db-wal / workstation.db-shm
  vault\<secret-id>\<version>.dpapi
  inbox\
  reports\
  logs\
  backups\
  plans\
  integrations\
```

`WORKSTATION_HOME` chooses the data directory. `workstation init --home D:\Workstation` persists a small non-secret locator in the user's Windows profile and prints the exact paths. The program supports other local drives; it must not assume every user has D:.

A missing/unmounted chosen drive produces `HOME_UNAVAILABLE`. **Never silently create a replacement home on C:**. Do not relocate the Windows profile, Windows credential infrastructure, another application's full AppData tree, or `TEMP` system-wide. Own temporary files belong beneath the configured home.

The home must be local, on a supported filesystem with validated permissions, and outside source repositories and cloud-synced folders. Reject a network/WSL-share database path. A permitted symlink/junction root must be explicitly registered by its resolved volume/path identity; unknown redirection fails closed. Reparse-point traversal is off by default. [S03][S08]

### 4.2 Existing `D:\Ops` and Docker scripts

The earlier PowerShell toolkit is an incident/prototype source, not an implementation to trust without inspection. Inventory its files, task registration, configuration, and observed behavior. Keep rollback copies. Initially run Workstation read-only alongside it.

Replace an existing entry point only after its behavior is covered by the new tests. A small PowerShell wrapper may preserve an old command. Do not register competing Docker starters or replace the project-specific local developer CLI.

Review the existing recovery scripts for stale error-file matching, broad process-name kills, unbounded child commands, global WSL shutdown, cross-volume movement of broken socket objects, indiscriminate error suppression, and recovery loops. Successful past execution does not remove these review requirements.

---

## 5. Evidence collection and ownership

### 5.1 Observation contract

Every collector result includes:

- collector ID/version, adapter version, host ID, start/end time, applicable product version;
- target IDs, requested scope, completed scope, skipped paths, truncation and permission errors;
- evidence type, sanitized values, source pointer, optional digest, observed time;
- coverage: `complete`, `partial`, `denied`, `unsupported`, or `timed_out`.

`0 matching records, complete coverage` differs from `0 records because collection failed`. Only the former supports a negative finding. Do not convert an unreadable directory into zero bytes.

A snapshot is not instantaneous: each observation has its own timestamp. Use monotonic clocks for deadlines/durations and UTC for persisted event times; render in the user's timezone. A clock change must not produce a false growth-rate calculation or an accepted expired plan.

### 5.2 Windows collectors

Collect process IDs, creation times, executable identities where available, parent IDs, owner SID, session IDs, working set, private commitment, CPU/I/O counters, handles and threads where supported. Persist only allowlisted command features, not complete raw command lines. Some fields require permissions or are unavailable; record unknown. Microsoft exposes process creation and lineage fields and explicitly warns that PIDs can be reused. [S06]

Use `(host identity, boot identity, PID, creation time)` as a process-instance identity. A parent edge is valid only when chronology is compatible. Before termination, open the process with the necessary rights, compare the current creation time, and operate on that held handle rather than resolving the PID again.

Distinguish working set, private committed bytes, virtual address-space size, and system commit. Do not add shared working sets and label the sum unique physical RAM. CPU utilization requires a second sample; unavailable deltas remain unknown.

File collection records logical size and allocated size separately when available, volume/file identity, link count, reparse type, and timestamp. Deduplicate overlapping roots/hard links for totals where identities are available. A sparse virtual disk's logical length is not a measured physical allocation. Do not hydrate cloud placeholders or follow WSL UNC paths during a normal scan.

### 5.3 Agent discovery

Resolve installed binaries/packages, known roots, user overrides, and running executable paths. Do not infer the GUI's effective environment from the inspecting shell alone. Codex documents `CODEX_HOME` and separate SQLite-location configuration; discovering a default folder is not proof it is the active one. [S16]

A registered adapter advertises independent capabilities: `inventory`, `storage_roots`, `session_metadata`, `native_probe`, `lifecycle_events`, `findings`, and supported typed repairs. The product records `supported`, `unavailable`, `reference_only`, or `unsupported_version` for each capability.

Native doctors are optional bounded subprocesses. Verify their supported commands/output schemas against the installed version and adapter tests. Do not assume all vendors provide `doctor --json`, that it is offline, or that it has no side effects. Network probes require explicit consent. A vendor doctor failure does not trigger a reinstall.

### 5.4 Ownership is an evidence graph, not omniscience

Edges use `confirmed`, `supported`, `suspected`, or `unknown`; these are categorical confidence labels, not calibrated probabilities. A process can have multiple consumers. Shared MCP services are not assigned to the last session observed.

Confirm associations through explicit registrations, known lifecycle events, documented session/process IDs, or compatible process creation lineage plus adapter evidence. Directory proximity, duplicate command names, elapsed time, low CPU, missing parent, or missing pull request are insufficient alone.

Closed windows and idle sessions are not proof of completed work. The default for uncertain ownership is protection. Missing events downgrade confidence rather than filling in a convenient history.

### 5.5 Lifecycle hooks

Hooks are optional and installed only after showing a diff and obtaining approval. Preserve existing hooks. Record a minimal allowlist of metadata; discard prompt/tool-output bodies before persistence. Hook entry points must return promptly and never invoke models, networks, repairs, or interactive UI.

Important: Claude's `WorktreeCreate` replaces normal Git worktree creation. Do **not** install it merely to observe worktrees. Use documented non-replacing lifecycle notifications and Git discovery instead. Hook absence or a missing `SessionEnd` is not definitive lifecycle evidence. [S17]

A bounded inbox can receive tiny deduplicated records when the database is busy. It has a size/age cap, no secret content, no arbitrary action field, and a reportable overflow counter. Hooks failing to record must not break the agent's work.

---

## 6. Health, fingerprints, and reporting

### 6.1 The pipeline

```text
bounded collection
 -> normalize and redact
 -> store observations
 -> infer evidence-bearing relationships
 -> evaluate versioned rules
 -> produce findings
 -> optionally plan an action
```

A finding has a stable type plus a distinct occurrence ID. It identifies affected objects, impact, severity, confidence, evidence IDs, missing evidence, scope, validity window, upstream references, and available action type. Unknown or untested versions may produce a generic warning but not an automatic version-specific repair.

The bundled fingerprint catalog is structured data with reviewed predicates. It has no shell fragments, dynamic code, executable URLs, or arbitrary regex evaluated without input/time limits. Remediation identifiers point to compiled typed operations. In v1 rules ship with signed releases; no separate hot-update script marketplace is needed.

Rule lifecycle: `candidate -> reproduced -> diagnostic_verified -> remediation_verified -> retired`. A source report proves someone reported a behavior; it does not prove every matching workstation has that root cause.

### 6.2 First diagnostic families

1. Low free space / observed growth at registered agent roots.
2. Repeated incomplete runtime staging directories.
3. Ambiguous or duplicated agent/helper installations and PATH entries.
4. Suspected retained helper/MCP processes, with shared-daemon exclusions.
5. Worktree residue, locks, missing registrations, and incomplete ownership.
6. Corrupt or unreadable supported state formats; never infer corruption from size alone.
7. Exact current Docker local-runtime socket failures.
8. Unsupported/inconsistent adapter state after an application update.
9. Incomplete resource mappings, expired verification, and conflicting decisions.
10. Own-tool database, storage, backup, and task-registration health.

Host telemetry may reveal a symptom without identifying its cause. `suspected process retention` is valid; `memory leak confirmed` requires evidence beyond one large sample.

### 6.3 Output

Human output groups urgent findings, protected active work, and one next action per finding. JSON carries full machine-readable coverage. Reports include when data was observed; cached GREEN is not presented as present health.

No aggregate score averages optional missing tools with dangerous runtime failures. Use severity plus coverage. `reclaimable_estimate`, `removed_logical_bytes`, and `observed_free_space_delta` are separate quantities; unrelated writes and sparse disk behavior prevent equating them.

---

## 7. Repairs: the safety-critical execution path

### 7.1 One plan protocol

```text
finding -> draft plan -> validated preview -> approval
 -> acquire action lock -> revalidate live targets
 -> preserve recoverable state -> execute narrow action
 -> verify postconditions -> journal result
```

Plans include schema/rule versions, occurrence/evidence IDs, exact target identities, host and user, protected roots, policy revision, required privilege, actions, preconditions, backup requirements, expected postconditions, expiry, retry bound, and a digest over canonical plan data.

Approval is bound to that plan digest and scope. Editing a target, provider, environment, executable, or rule invalidates it. Recheck even unexpired plans: a new active session, changed file identity, replacement process, changed Git HEAD, or new dirty content blocks the action.

Application approvals prevent accidental and compliant-client misuse. They are **not** an authentication boundary against arbitrary malicious software running as the same Windows user. Do not market console confirmation or a local token as protection against a fully compromised user session.

### 7.2 Typed actions

Allowed families include: copy a bounded known file to an owned backup; rename one validated inactive runtime directory in the same parent; restore a file only when the current version matches the expected post-repair identity; run a reviewed vendor lifecycle operation; remove a confirmed reproducible cache after a specific plan; perform a non-force Git worktree lifecycle operation; update the product's own metadata; and stop an individually identified process only with explicit impact approval.

There is no generic `execute_shell`, arbitrary PowerShell payload, `delete_tree(path)` exposed to MCP, or unrestricted elevated script.

Distinguish:
- **Reversible state change:** a restored registry/config from a verified backup, with no concurrent writes.
- **Recoverable deletion:** content exists in an independently verified backup and a documented restore procedure.
- **Irreversible process stop:** unsaved in-memory state cannot be restored.
- **Reproducible cache removal:** not rollback; rebuilding may take time/network access.

### 7.3 Concurrency and crash consistency

Serialize actions per host/home and per affected application/repository. Hold no long SQLite write transaction while a user approves or a subprocess runs. Persist `planned`, `approved`, `running`, step results, and terminal status in short transactions. Every step has an idempotency key.

On interruption, reconcile observed post-state; do not blindly replay a kill, removal, or credentialed command. Unknown completion is `interrupted_needs_review`. For file replacement, stage on the same volume, flush, validate, and replace with supported Windows APIs; no assumed transaction spans the filesystem and SQLite.

### 7.4 Bounded command runner

Resolve full executable paths from verified installations; do not use current-directory search. Pass argument arrays, not interpolated shell strings. Set explicit working directory, environment policy, stdin behavior, output cap, deadline, and permitted network/interactive behavior. Concurrently drain stdout/stderr. A timeout cancels only the subprocesses the tool owns, not all matching process names.

Short-lived probes can use a private Job Object where tested. Long-running user commands must not accidentally inherit probe kill-on-close behavior. General managed launching of existing agents is excluded from v1; Microsoft documents nested-job and breakaway constraints. [S07]

Repository-based commands can execute hooks, fsmonitor programs, filters, or project scripts. Pure inspection must avoid invoking project-controlled helpers where possible, reject untrusted configurations, and never use an alias supplied by a repository. A command label such as `test` or `doctor` does not make it intrinsically safe.

### 7.5 Docker recipe

Inspect context/config and verify the actual local Desktop endpoint. `DOCKER_HOST`, `DOCKER_CONTEXT`, and explicit CLI flags can choose another daemon; never rely on the current context name or a generic `docker version` result. [S11]

Match a fresh backend error from this startup attempt, the exact failing path, the expected runtime directory identity, and the observed Desktop state. A months-old error file or any mention of `.sock` is insufficient.

Attempt graceful stop through a supported bounded operation. Do not force-stop a healthy engine or an uninspected set of containers. If the engine cannot be inspected, require explicit interruption approval rather than interpreting unavailability as an empty workload.

After confirming owning processes have stopped, isolate only the validated runtime container in place on the same volume. Moving a damaged reparse-point directory from C: to D: is not a reliable rename and is not the primary operation. Keep an owned recovery manifest on D: pointing to the in-place quarantine.

Never touch data VHDXs, images, volumes, databases, global Docker data, or unrelated application state. Do not invoke `wsl --shutdown`: Microsoft defines that operation as terminating every running distribution. Even targeted distro termination is an interruption requiring scope/impact review. [S12]

One recovery attempt, one bounded verification interval, then stop and report. Repeated recovery is not a permanent upstream fix. Any opt-in startup recovery reuses the same engine and existing user-approved task; no competing startup task is installed.

### 7.6 Codex attachments recipe

Discover the actual active home and supported attachment-registry layout. Confirm the owning applications are closed and the failure evidence is current. Back up only the actual registry; preserve every attachment file and session reference at its original path. Do not rename the entire attachments tree.

A missing registry is not automatically an error. Invalid JSON, stale references, access denied, absent parent directories, and oversized-but-valid state are different conditions. Use only the recovery supported by that tested adapter/version; otherwise emit instructions, not mutations. A successful file operation is not a successful large-paste workflow: UI verification remains a separate user-confirmed acceptance step.


## 8. Workspaces and worktree lifecycle

### 8.1 Discovery and identity

Use registered repository roots and bounded discovery in user-approved folders. Run Git's stable machine formats, including `git worktree list --porcelain -z` and porcelain status. Associate linked worktrees through the common Git directory, not a guessed folder name. A separate clone is a separate checkout; identical remote URLs alone do not justify merging project identities. [S18][S19]

Track local repository identity, common directory, checkout path/file identity, branch or detached HEAD, upstream observation, ahead/behind as of the last known refs, locks, submodule presence, session edges, protections, and measured storage. Record remote-ref freshness. Never silently fetch or interpret stale refs as proof work is pushed or merged.

Worktree and project associations are many-to-many when a session spans repositories. Do not force a monorepo service, a Git repository, and a project to be identical objects.

### 8.2 Protection and leases

Supported states: `active`, `protected`, `candidate`, `blocked`, `retired`. A user can explicitly pin a workspace or register a task lease with host/process/session identifiers. Leases help coordination between participating clients; they do not prove external clients have stopped. Expiry removes a protection signal, not proof of safe deletion.

Block cleanup for current working directories, tool/ancestor workspaces, known active sessions, running owners, unresolved handles, untracked/ignored unknown files, dirty state, unique commits, ongoing merge/rebase, submodules, unknown coverage, or absent/unmounted volumes. The product does not automatically unlock Git locks.

### 8.3 The cleanup standard

Before an entire worktree can be removed, require all of:

- exact repository/worktree identity and complete inspection;
- owner/application quiescence or explicit operator acceptance of the interruption risk;
- no tracked modifications, no unmerged entries, and inventory of untracked **and ignored** files;
- preservation of `.env`, local databases, uploaded assets, generated unique artifacts, and anything not proven reproducible;
- local-only/detached commits preserved through a durable Git ref or verified backup; no automatic branch deletion;
- current approval tied to HEAD and directory identities; recheck immediately before execution;
- a tested vendor/Git removal operation without `--force`.

Git documents clean-worktree restrictions, `lock`, `move`, `repair`, and that `prune` removes administrative information rather than being a universal cleanup command. Git status excludes ignored files unless requested. Therefore “Git status is clean” is not sufficient proof that the directory contains no valuable non-Git data. [S18][S19]

Initially offer candidate review and individually approved cache cleanup. Enable whole-worktree removal only after negative safety fixtures pass. Do not invent a universal “unused after N days” policy. Archiving and cleanup are separate choices.

### 8.4 Worktrunk integration

Discover the real binary. On Windows Worktrunk documents `git-wt` because `wt` can resolve to Windows Terminal. Do not launch whichever `wt.exe` appears first. Inspect integration hooks and side effects before using a lifecycle command; `wt remove` may also remove a branch, so it is not interchangeable with our narrower worktree-only plan. [S13]

Workstation provides ownership, protection, evidence, and inventory. Worktrunk remains an optional workflow tool. Existing Worktrunk hooks and shell configuration are preserved.

---

## 9. Chronicle: progression, decisions, and supersession

### 9.1 Two kinds of history

**Operational history** records observed changes: process appeared/disappeared, workspace created, version changed, incident detected, repair attempted, resource verified, or a Git commit imported.

**Project reasoning history** records ideas, alternatives, experiments, assumptions, accepted decisions, rejections, supersession, reversals, releases, and unresolved questions.

A commit proves a code change existed; it does not establish the author's rationale. An assistant saying “we decided” is a claim, not authority. A suggested architecture is a proposal until approved.

### 9.2 Record model

A decision revision contains: immutable ID; stable topic key; project; explicit scope (environment/component/branch where relevant); status; statement; rationale; alternatives; consequences; source IDs; author; approval actor/time; effective time; recording time; and optional predecessor/relation.

Statuses: `proposed`, `accepted`, `rejected`, `withdrawn`, `superseded`. Facts or assumptions additionally carry verification/freshness state. A reversal is a new accepted revision linked to the previous one, not deletion of history.

Keep **effective_at** (when the decision applies) distinct from **recorded_at** (when this tool learned it). A retrospective import must not rewrite what the tool knew earlier. Support queries such as “effective on September 13” and “as recorded by September 15.” Simple temporal columns and lifecycle events suffice; no graph database is required.

Use Markdown ADR import/export as a durable human-readable format. MADR is an established structured decision-record convention; adapt its rationale/alternatives concepts rather than importing a heavy knowledge platform. [S20]

### 9.3 Supersession rules

Accepting a replacement is one database transaction: validate project/topic/scope, reject cycles, record new acceptance, link predecessor, and update the derived current-head projection. Preserve the old statement and rationale.

The default policy permits one accepted revision-chain head for an exact topic/scope. The chain head is not necessarily the decision effective today: future-effective acceptance must leave the previous decision in force until its effective boundary. As-of queries evaluate the acceptance/supersession ledger against both effective and recording time, not only the head pointer. Concurrent or incompatible imports become `conflict_needs_review`, not “the newest timestamp wins.” Overlapping global/component/environment decisions must carry explicit override relations; absent an override, expose the conflict.

Human-approved policy outranks imported claims. An LLM can draft a proposed decision with citations but cannot accept it, supersede an accepted one, grant permissions, modify secrets, or classify destructive targets.

### 9.4 Ingestion without surveillance

Default sources: manually entered decisions, approved project manifests, Git metadata, ADRs, and the tool's own events. Session metadata is collected only through enabled adapters. Transcript/Markdown imports require explicit path/project scope; selected excerpts may be retained only with content-import consent.

A content importer streams bounded data, strips credential-like content before persistence, stores source identity/digest/locator and consent metadata, and creates proposals rather than silently authorizing decisions. Partial or unavailable source content remains flagged. Do not claim to reconstruct every past conversation.

Optional model-assisted extraction is a separate user-triggered job with source preview, provider choice, cost/token cap, and no credential access. The whole feature works without it: users or their existing builder can submit structured proposals for review.

Entire may supply checkpoint/session evidence; Codbash may supply navigation where supported. Workstation does not replicate their full replay or transcript storage. [S14][S15]

### 9.5 Context packet

`workstation context --project <id>` returns a bounded packet:

1. project identity and environment;
2. accepted applicable decisions, with sources and superseded warnings;
3. unresolved conflicts and unknowns;
4. active/protected workspaces and tasks;
5. verified resource references and freshness;
6. recent milestones/incidents and proposed changes clearly marked.

Never include secret values, entire transcripts, another project's data, or instructions extracted from untrusted sources as execution authority. Default to roughly 8 KiB of text and explicit pagination/source links. This solves repeated context archaeology without building a general chat product.

---

## 10. Resource Atlas

### 10.1 Scope and schema

Entities: project, environment, service, endpoint, repository, deployment target, database/resource identifier, documentation/runbook, owner, and credential reference. Store provider identity, opaque provider object ID, human label, declared role, source, approval, verification type/result, observed time, and freshness policy.

Environment is always explicit for operational queries. `production`, `staging`, `development`, and `local` are distinct identifiers, not aliases inferred from names. No fallback from an unknown production credential to a development or broader-scope credential.

A project can use multiple repositories/services. A service can depend on multiple resources. Resource IDs are local stable IDs with provider IDs as external identifiers. This is a small service catalog, not a hosted internal developer platform; Backstage documents the analogous service/resource/owner concept. [S21]

### 10.2 Import and authority

A project `.workstation/project.json` describes non-secret facts and relative repository paths. Reading it creates untrusted/proposed records until the user approves the source and changes. It cannot contain effective policy grants, automatic tasks, arbitrary shell commands, credentials, or permission overrides.

Persist accepted revisions locally. A changed manifest creates a reviewable delta, not a silent replacement of the production URL. Keep user-confirmed and provider-observed facts separate; disagreement is an explicit conflict.

### 10.3 Verification

A verification result states what was actually tested: syntax valid, DNS resolved, TLS connected, configured health route responded, provider returned matching object ID, or human confirmed the mapping. These are not interchangeable.

Network checks are off in ordinary offline diagnosis. Manual verification targets an approved endpoint, has a deadline and response-size limit, validates TLS, forbids cross-origin redirects by default, and never attaches credentials inherited from the host. Deny URL userinfo and secret-bearing query fields. Reject automatic metadata-service/link-local requests and arbitrary discovered internal endpoints; user-owned private targets need explicit registration. No web crawler is required.

A healthy public endpoint proves neither deployment ownership nor authorization to mutate it. Resource records do not grant operational permission.

### 10.4 Credential references

Use a provider-neutral **structured** record rather than parsing an invented universal URI:

```json
{
  "provider": "doppler",
  "connection_id": "provider-dev",
  "project_id": "example-app",
  "environment_id": "development",
  "locator": {"config": "dev_agent", "key": "DATABASE_URL"},
  "required_for": ["local-api"],
  "expose_to_agent": false
}
```

Provider-native locators such as 1Password secret references are retained when useful. A display label such as `secret://...` is our identifier only, not a claim that all providers recognize that scheme. Never put values in resource tables or manifest examples.

---

## 11. Secrets architecture and threat model

### 11.1 The promise

**Reduce accidental exposure and storage sprawl; use established providers; make the required credential discoverable without displaying it.** This is not a secrets-access sandbox for a malicious agent running with unrestricted rights as the same user.

DPAPI normally ties decryption to the same user's logon credentials and computer, with documented exceptions such as roaming profiles. That protects stored ciphertext under its actual Windows threat model; it does not distinguish two programs using the same user identity. Use user scope, not `CRYPTPROTECT_LOCAL_MACHINE`. [S09]

Environment injection keeps values out of static project files by design, but the child can read, log, transmit, and pass them to descendants. A TTL on approval does not revoke a value already delivered. An agent able to modify the executed script can make the program reveal it. Windows documents environment inheritance, and Doppler and 1Password expose environment-based execution patterns. [S10][S22][S23]

Higher assurance requires an external credential broker, separately isolated identity/process, or remote narrowly scoped operation. Those are outside this local v1. Production access defaults to external scoped providers and explicit per-run approval.

### 11.2 Local DPAPI store

The local vault stores immutable ciphertext files under `vault/<id>/<version>.dpapi`; metadata stores only IDs, labels, scope, timestamps, version, and the blob digest. The content is encrypted with user-scoped `CryptProtectData`; optional descriptions contain no secret value. Do not invent key derivation, a master password protocol, or a cloud key service.

Input uses a hidden interactive prompt or protected stdin, never a CLI argument. Prevent plaintext `Debug`/serialization on secret types; use short-lived buffers and best-effort zeroization. Do not promise memory wiping against all allocator copies, crash dumps, administrators, or compromised runtimes.

Write new ciphertext atomically, verify its integrity/decryptability in-process, then commit the new metadata reference. A crash may leave an unreferenced encrypted blob; reconcile it without discarding referenced versions. Do not rotate a secret in a remote service merely because a local vault version changed.

The vault directory and metadata receive restricted ACLs for the owning user and necessary Windows administrators/system operations. DPAPI key material remains managed by Windows; it is not moved to D: by this tool.

**Recovery warning:** copying encrypted blobs to another computer is not a portable-vault backup guarantee. Metadata export and ciphertext backup are separate. On initialization, require acknowledgment of the restore limitation and recommend external-provider recovery or a Windows-account/system backup. No home-grown portable encryption format in v1. A tested old-to-new provider migration can be explicit, value-in-memory only, and never silently deletes the source.

### 11.3 Provider boundary

A `SecretProvider` advertises `metadata`, `resolve`, `exec_scoped`, `set`, and `health` independently. Capabilities not supported by the installed provider/version fail clearly. Metadata discovery must not fetch all values as a shortcut.

Prefer provider-managed authentication and documented scoped identities. No importing all browser/CLI credentials or giving the agent a user's master provider token. Doppler, 1Password, Infisical, and Bitwarden have documented CLI paths, but their scopes, metadata APIs, caching, interaction, and injection behavior differ. [S22][S23][S24][S25]

V1's common contract supports references to all four. Doppler/1Password execution is delivered first. Other adapters must pass identical privacy and environment-selection tests before enabling `resolve`/`exec_scoped`. Authentication remains in each provider; the product does not implement four login systems.

Some providers support native process execution; others require resolving selected values into protected memory. Prefer native execution when its selection and output policy meet the approved plan. Do not use “download all secrets” merely to run a task needing one key. Explicitly account for provider fallback caches and prevent plaintext intermediary files.

### 11.4 Credentialed task protocol

A locally approved task identifies project, environment, exact executable, argument vector, working directory, approved script/config identities, required secret references, timeout, and output policy. Example command:

```text
workstation run --task local-api
```

A task descriptor is stored in locally approved policy, not silently enabled by the repository manifest. Changing executable, argument vector, environment, script identity, or credential set invalidates approval. Do not infer that `npm test`, `migrate`, or `read-only-check` is actually read-only; authorization is to give those values to the program and code it loads.

Construct a fresh minimal child environment; resolve only required references; remove provider bootstrap tokens and unrelated inherited credentials; use a fixed working directory and trusted interpreter resolution. Never temporarily add secrets to the global user environment. No shell-string concatenation.

Default output for credentialed tasks is status/exit metadata only. Interactive output may be allowed explicitly. Redaction of exact secret values is defense in depth, not a guarantee against encoding or transformation. Never claim that masking prevents deliberate exfiltration. No plaintext secret in normal logs, reports, process arguments, decisions, or MCP responses.

Raw `reveal` is omitted from v1. Provider-native human tools remain available outside Workstation when needed. MCP has neither resolve nor execute capabilities.

### 11.5 Secret hygiene discovery

Opt-in, project-scoped scans inspect selected configuration files for likely secret fields. They report source path class and key names, not values. A variable named `PASSWORD` is not proof of a live credential. Report `suspected plaintext secret`, confidence, and coverage.

Do not blanket-read browser stores, user vaults, all CLI configs, or whole disks. Migration is a separate approved operation. Verify the destination and application readback before offering source-file edits; backups can themselves contain secrets and need explicit encrypted handling. The hygiene scanner must not become a new secret collection database.

---

## 12. Capabilities and contextual resource discovery

**Accepted amendment:** [FS-1.0-D1](FS-1.0-D1.md). Implement within Capabilities at **R4**;
certify at R5; **R0 is unchanged**. The amendment is normative for this section.

The question is now: “What existing resource could materially help with this work, including
something I may not know exists?” Resources include existing features, tools/integrations,
skills, reusable workflows, documentation, reference implementations, datasets and benchmarks.
Discovery is not restricted to broken environments or installable tools.

Use approved focus/project evidence, applicable decisions, scoped constraints and actual inventory
coverage. Show at most three justified suggestions inside ordinary cached context and project
reports. Prefer applicable existing options; honor project/need-scoped dismissal and snoozing.
No need may be inferred solely from a stack marker or popularity. Unknown compatibility stays
needs-review; incomplete inventory never becomes absence.

Ship 20–30 genuinely reviewed initial entries as versioned local data. Permit an optional minimal,
reviewed research brief for the user's browsing-capable agent and explicit bounded candidate import
into an unverified queue. No crawler, mandatory LLM, synchronous web search, new service or automatic
agent launching. Synthetic examples in the source package are not the reviewed catalog.

A suggestion is not an Atlas association or a Chronicle decision. Feedback is not installation
approval. Material remains inert metadata; skill scripts/full instructions never enter watched
directories merely from discovery. Only existing certified integrations can enter plan/preview/
approval/revalidation/verification. A new catalog entry adds no executable action.

Contracts and acceptance extensions are in FS-1.0-D1 and FS-1.0-D1-ACCEPTANCE.md. Preserve separate
claims for source existence, applicability and demonstrated benefit. Record review freshness and
resource-specific licensing rather than repository-wide reassurance.

---

## 13. Public interfaces

### 13.1 CLI

```text
workstation init --home D:\Workstation
workstation doctor [--deep] [--native] [--json]
workstation project add <path>
workstation project show <id>
workstation workspace list [--project <id>]
workstation workspace protect <id> --reason <text>
workstation timeline --project <id> [--since <time>]
workstation decision propose --file <record.json>
workstation decision accept <id> [--supersedes <id>]
workstation resource list --project <id> --env <id>
workstation resource verify <id>
workstation secret set <reference-id>
workstation run --task <approved-task-id>
workstation capability explain <id>
workstation capability connect <id> --project <id>
workstation context --project <id> [--json]
workstation explain <finding-id>
workstation plan <finding-id>
workstation apply <plan-id>
workstation reclaim [--project <id>]
workstation report [--html] [--share]
workstation backup <destination>
workstation restore <backup-id>
workstation schedule enable|disable
workstation mcp --project <id>
```

`reclaim` generates candidates/plans; there is no global delete switch. `apply` handles reviewed typed plans only. `doctor --native` may execute documented vendor diagnostics and discloses their interaction/network behavior. `report` reads cached observations and shows freshness. No command called `repair-all` or `fix-everything` exists.

JSON envelope: `schema_version`, `operation`, `run_id`, `observed_at`, `status`, `coverage`, `data`, `warnings`, `errors`. stdout is JSON only in JSON mode; stderr carries sanitized diagnostics. No colors in machine output.

Exit codes: `0` command completed successfully (may contain warning findings); `2` requested check failure or operation blocked; `3` partial/unsupported inspection; `4` invalid input/config; `5` user cancellation; `6` interrupted/indeterminate execution. A `--fail-on warning|critical` option makes diagnostic CI behavior explicit. Credentialed task output records the child's actual exit separately.

### 13.2 Read-only MCP

Expose project-scoped tools/resources for `project_context`, `workspace_status`, `current_decisions`, `project_timeline`, `resource_lookup`, `health_latest`, and `capability_explain`. Read cached approved data. No arbitrary path reads, SQL query endpoint, raw transcripts, live network fetch, plan approval, secret resolution, executable launch, filesystem mutation, or process termination.

Use stdio, no inbound TCP listener. Exit on stdin closure/client death, cap messages, bound responses, and avoid polling. stdout is protocol traffic only, as the MCP documentation requires. [S26]

Connection is explicit per client/project, with configuration diff and rollback. The MCP project's scope is not a sandbox against a client with unrestricted filesystem access; it is the API's disclosure boundary. Imported material is returned as untrusted evidence, not higher-priority instructions. Follow MCP security guidance for trust and authorization boundaries. [S27]

### 13.3 Human overview

Generate static HTML with inline local assets, no external scripts/fonts/CDNs, escaped text, no raw HTML from logs, no clickable executable links, and a timestamp. It is a report, not a background dashboard. The five sections share project navigation and show what is known, protected, stale, and missing.

The report does not reveal credentials or raw provider tokens. A share report is built from an allowlist of fields, not a blind redaction pass over the full database. Sharing is an explicit export; no auto-upload.

---

## 14. Persistence and data model

### 14.1 Database groups

| Group | Core tables |
|---|---|
| Identity | projects, repositories, project_repositories, environments, installations, objects |
| Evidence/health | scan_runs, observations, relationships, findings, finding_evidence |
| Workspaces | workspaces, sessions, protections |
| Chronicle | sources, events, decisions, decision_sources, decision_transitions, decision_heads |
| Atlas | resources, resource_verifications, secret_refs |
| Operations | plans, approvals, action_runs, action_steps, audit_events |
| Integration | capability_connections, approved_tasks |

The included `schema/001_initial.sql` is an executable baseline for these contracts, not evidence of a complete application. Domain code adds versioned validation, overlap checks, approval provenance, path identity, and runtime authorization. JSON columns hold bounded typed adapter metadata, not a dumping ground for raw transcripts or credentials.

Use explicit foreign keys and unique identities; objects supply graph endpoints. Many-to-many relationships carry provenance/validity, so shared resources are not falsely exclusively owned. Numeric byte/time values declare units. IDs are opaque local IDs; provider/vendor IDs remain separate.

### 14.2 SQLite operation

Enable foreign keys per connection. Use WAL only on local supported storage, short writer transactions, a bounded busy timeout, and a per-home writer lock. Readers do not retain transactions across user interaction or external commands. No concurrent uncoordinated checkpoint loops. [S03]

Use a patched bundled SQLite release and record actual runtime/source ID. SQLite documents that its WAL-reset bug is fixed in 3.51.3 and later and in specific backports; do not accidentally bundle an older engine through a transitive library. Do not choose the withdrawn 3.52.0 release as the production pin. [S04]

Back up the product database using SQLite's backup mechanism or a fully quiesced supported procedure; copying only a live `.db` while ignoring WAL is not a backup protocol. Validate restore into a separate directory first. [S28]

Do not alter vendor databases. Prefer supported export/native API. If a private store must be read, require a tested read-only path that does not create or change sidecars; otherwise wait for the application to close and obtain a consistent snapshot. Unknown schema fails closed. Never run `VACUUM`, migration, repair, or `PRAGMA journal_mode` against another application's database.

SQLite FTS5 provides optional lexical search over approved decision/event text; no vector service is needed. It is not semantic multilingual search. Preserve Unicode source text, test Arabic/English paths and content, and keep normalization in a separate index. [S29]

### 14.3 Retention and storage guardrails

Initial product budgets are tunable design targets:

- own logs: 50 MiB total, 14 days;
- sanitized report snapshots: 100 MiB total, 30 days;
- observation details: 14 days; compact daily aggregate metrics: 90 days;
- hook inbox: 10 MiB, bounded dedupe/expiry; overflow reported;
- automatic backup budget: 512 MiB, with protected required backups never silently discarded;
- approved project decisions/resources and their essential provenance: retained until explicit archive/delete;
- no transcripts imported wholesale by default;
- own working metadata target: below 1 GiB for the declared reference workload.

If the budget is exhausted, stop optional collection and warn; do not delete indispensable history, vault versions, or pending-repair backups to continue. The program must audit its own growth. Same-volume quarantine does not free space and must never be advertised as reclaimed bytes.

---

## 15. Threat boundaries and failure behavior

| Threat/failure | Required behavior |
|---|---|
| Prompt injection in README/log/session/MCP output | Treat it as data; cannot authorize a plan, install a tool, change scope, or request a secret |
| Malicious project manifest | Strict schema; declarative only; approval required for changed accepted facts; policy never imported |
| Secret in argv, URL, command output, transcript | Allowlist persisted fields; redact before storage; suppress credentialed output by default; no blanket secrecy claims |
| PID reused / executable replaced | Creation identity and held process handle revalidation; block on mismatch |
| Junction/symlink swapped / unknown volume | Resolve and validate identities, inspect path components; no arbitrary traversal; block ambiguous target |
| Legitimate idle/shared daemon | Show attribution and uncertainty; do not kill based on age/CPU/duplicate count |
| Agent starts between preview and action | Revalidate immediately; quiescence/explicit scope needed; no claim that cooperative locks constrain other software |
| Database locked | Bounded retry and partial result; never delete lock/DB to proceed |
| Home drive missing/full | No C: fallback; read-only/blocked mode with explicit error |
| Adapter format changed | `unsupported_version`; keep generic evidence; no guessed mutation |
| Provider unavailable/auth expired | Report exact metadata state; no broad fallback token, secret file, or wrong environment |
| Same-user malware or unrestricted agent | Outside DPAPI/application-policy isolation promise; recommend external scoped identity/isolation |
| Administrator/system compromise | Outside local integrity/confidentiality guarantee |
| Interrupted action | Reconcile exact state, preserve audit/backup, no blind replay |
| Old error log | Mark stale; cannot trigger recovery |
| Healthy remote Docker context | Reject as a local Desktop recovery target |
| Public/third-party resource link | Do not infer ownership or network authorization |
| Own tool crash / unavailable home | Other agents continue; no indispensable daemon or control-plane dependency |

Local audit is best-effort accountability and recovery evidence. A hash chain alone would not make it tamper-proof against the same user or administrator; v1 makes no tamper-proof audit claim.

---

## 16. Performance targets and self-reliability

Targets, to be measured on a documented reference machine/workload:

| Operation | Initial budget |
|---|---|
| Cached project context | <= 1 second p95; default <= 8 KiB output |
| Normal doctor | <= 10 seconds p95 for 20 registered repos, 200 worktrees, 500 relevant processes; clearly report partial work |
| Deep scan | 120-second overall default; per-root time/file budget; user can explicitly extend |
| Ordinary probe | 5-second default subprocess deadline; adapter-specific hard ceiling |
| Hook | <= 250 ms target; 1-second hard cutoff; no model/network/repair |
| Basic read-only MCP request | <= 1 second p95 cached; no implicit deep scan |
| Concurrent subprocess probes | At most 2 initially |
| Background mode | No resident agent by default; optional scheduled read-only command only |

A partial scan is better than an indefinite hang. The UI must show what was omitted. Do not launch every vendor runtime during normal `doctor`. No update checks on every invocation.

The optional scheduler adds one user-level health task after consent, skips overlaps, and produces no pop-up storm. Existing Docker recovery startup is distinct and must not be duplicated. No unattended Codex state repair, worktree removal, secret access, or process termination. A saved opt-in Docker recovery policy is permitted only for its separately reviewed exact recipe.

---

## 17. Repository and development structure

```text
workstation/
  Cargo.toml
  Cargo.lock
  rust-toolchain.toml
  README.md
  SECURITY.md
  CONTRIBUTING.md
  crates/
    workstation-core/src/
      model/ evidence/ ownership/ fingerprints/
      policy/ plans/ chronicle/ atlas/ context/
    workstation-platform/src/
      windows/ filesystem/ git/ storage/
      runner/ agents/ providers/ actions/
    workstation-cli/src/
      commands/ render/ mcp/
  schema/
    migrations/
  contracts/
    v1/
  fingerprints/
    codex/ claude/ cursor/ copilot/ substrate/
  capabilities/
  fixtures/
    public-sanitized/ synthetic/
  tests/
    contracts/ negative/ integration/ windows/ e2e/
  docs/
    SCOPE-LOCK.md
    ARCHITECTURE.md
    THREAT-MODEL.md
    ADAPTER-CONTRACT.md
    REPAIR-AUTHORING.md
    RELEASE-GATES.md
    RECOVERY.md
  packaging/
    windows/
  .github/workflows/
```

Do not prepopulate empty enterprise/macOS/cloud/plugin frameworks. Start with one vertical slice and add modules when their release gate is being implemented. A trait is justified by a real current backend or tested fake, not speculative extensibility.

Agent and provider adapters expose typed capabilities. Each includes fixtures for supported layouts/versions, known unsupported conditions, parser limits, and evidence requirements. Tests must distinguish synthetic success from real Windows/app success.

---

## 18. Build sequence and acceptance gates

### R0 — foundation and read-only proof

Deliver initialization, path policy, bounded runner, SQLite migration/backup, own health, process/storage collection, registered projects/Git worktree inventory, JSON/report output, and three real incident fixtures from the user's experience. No repairs or secrets yet.

Gate: normal-user Windows build; no external mutations during `doctor`; incomplete scans remain partial; disconnected D: does not create data on C:; no orphan collector children; no secret canary in outputs; tested schema/runtime version.

### R1 — reliable diagnosis and workspace protection

Deliver the first four diagnostic adapters, explicit coverage matrix, evidence graph, protections, candidate cleanup, fingerprint explanations, context skeleton, and current Docker endpoint recognition. Include both healthy and failure fixtures for every rule.

Gate: existing active Codex work and legitimate Node/MCP servers remain protected; PID reuse, dead parents, missing events, ignored files, detached commits, and unknown schemas do not authorize removal.

### R2 — selected repair plans

Deliver preview/approval/revalidation/journal, targeted offline Codex registry recipe, exact Docker runtime recipe, and individually approved reproducible-cache cleanup. Whole-worktree removal is enabled only after its full gate; otherwise retain plan-only behavior.

Gate: crash after every action step is recoverable/reported; changed targets revoke the plan; remote Docker endpoint rejected; no global WSL shutdown; process interruption is disclosed as irreversible; repeated failure stops.

### R3 — Resource Atlas and secrets

Deliver project/environment/resource manifests with review, metadata verification, local DPAPI, provider-neutral references, Doppler/1Password adapters, credentialed task protocol, and opt-in targeted hygiene findings. Infisical/Bitwarden execution remains disabled until its certification passes.

Gate: wrong environment blocked; only requested secrets resolved; no provider token inherited unnecessarily; no credential in logs/argv/context/MCP; real same-account restore and different-account negative tests documented; non-portability clearly explained; malicious same-user threat is not misrepresented as solved.

### R4 — Chronicle and contextual discovery (FS-1.0-D1)

Also implement the accepted D1 catalog, deterministic matching, scoped feedback, ordinary-context
cards and reviewed handoff/import. Complete the D1 acceptance additions; R0 remains unchanged.

Deliver event history, accepted/proposed decisions, scoped supersession, evidence import, conflict queue, lexical search, current-context packet, contextual capability guidance, Entire references/import where supported, and Codbash navigation.

Gate: future-effective and retrospectively recorded decisions answer correctly; conflicting agent proposals do not overwrite approved state; import is opt-in; deleted evidence is shown unavailable; no inferred rationale presented as a fact.

### R5 — integration and v1 certification

Include D1 offline/latency, candidate-import, renderer/MCP, source-revision and authority-boundary
tests. Specification checks are not native product pass counts.

Deliver optional read-only project MCP, static overview, signed/verified packaging, upgrade/restore flows, optional scheduled check, documentation, clean install/uninstall behavior, and fixture corpus.

Gate: fresh Windows VM, upgrade VM, actual machine coexistence, offline use, low disk, missing drive, non-ASCII paths, parser fuzzing, redaction, and adapter compatibility tests pass. Publish exact supported builds and limitations. A single successful recovery or task exit code is not broad certification.

---

## 19. Acceptance fixture matrix

The companion `ACCEPTANCE-MATRIX.md` specifies the minimum suite. High-value cases include:

- the current Codex session hosts the collector; never close that agent to fix its registry;
- a long-idle legitimate database or MCP daemon with no parent;
- a stale PID immediately reused by an unrelated process;
- partial process visibility due to permissions;
- a dirty/untracked/ignored `.env` in an apparently merged worktree;
- detached/local-only Git commits and a repository-wide stash;
- a drive temporarily unavailable rather than deleted;
- junction escaping an allowed cache root; reparse swap between plan/apply;
- Docker healthy on a remote daemon, current local engine unavailable;
- a stale `backend.error.json` with a healthy current Docker instance;
- one correct secret reference and an identically named secret in production;
- a program printing, encoding, or forwarding an injected secret;
- resource URLs containing userinfo/tokens and unsafe redirects;
- an untrusted manifest instructing automatic installation/approval;
- two simultaneous decisions superseding the same current head;
- retroactive effective time vs recording time;
- malformed JSONL, very large lines, Unicode/bidirectional-control paths, invalid UTF-8 input, and HTML injection;
- scanner timeout, subprocess output flood, database contention, power-loss/action interruption, and disk-full writes.

Measure false positives, verified incident recall on the labeled corpus, blocked unsafe actions, command latency, maximum resource consumption, and fresh-scan coverage. Do not quote global reliability prevalence or extrapolate a fixture result to all users.

---

## 20. Distribution, updates, and backup

Ship a portable Windows release with checksum, manifest, dependency/SBOM record, and Authenticode signing when the publishing identity is available. Do not claim signing until verification exists. A conventional installer/winget manifest can follow; no custom updater service.

Updates are explicit. Download to the chosen drive, verify publisher/digest, record the prior binary/schema, back up with a supported SQLite procedure, and apply forward migrations. Abort safely on insufficient disk. A failed migration retains the old database and backup. Downgrades across incompatible schemas require restoring a matching database snapshot rather than opening it blindly.

Own-tool uninstall removes its task/configuration integrations and executable only. Retain metadata/vault by default with explicit export/delete choices. Never remove third-party tools, workspaces, Docker data, provider credentials, or project hooks not installed by this product. Integration receipts identify the exact changes to undo without overwriting later user edits.

Backup receipts include database schema/build, integrity result, file manifest/digests, DPAPI identity warning, and content sensitivity. Restore first validates into a separate location. Metadata restore and vault decryptability are separately tested. Old repair backups are not silently pruned while an incident or rollback depends on them.

---

## 21. Example end-to-end workflows

### Starting work

The user registers a project and approves non-secret resource/decision imports. `workstation context --project demo` prints current decisions, protected worktrees, resource references, known limitations, and recent changes. The agent receives the same bounded metadata through JSON or read-only MCP. No credentials are revealed.

### A workstation slows down

A normal scan records process/resource evidence, recognizes a probable family of retained MCP helpers, but distinguishes a shared live server. It explains the evidence and missing ownership links. A suspect not sufficiently attributed remains review-only. An approved action acts on exact process instances and verifies them individually; it does not kill `node.exe` globally.

### An old worktree occupies disk

The tool finds the worktree through Git, discovers ignored `.env` and local-only commits, and blocks removal. It may propose only a verified reproducible build-cache cleanup. After the user preserves local assets and explicitly closes owners, a new plan can safely consider the worktree itself. An old plan is not reused.

### A decision changes

A builder submits a proposal with source references. The user accepts it as a scoped replacement. Chronicle atomically records supersession and retains the old rationale. A context packet now returns the new accepted decision and notes the old one is historical. A different environment can retain a different accepted decision without global overwrite.

### An agent cannot find a credential

Atlas returns the matching project/environment reference, provider, last verification, and available approved task. It returns no value. The user runs or authorizes the exact task. The provider resolves only its required values; the program receives them under the stated same-user/process risk. Production is never inferred from a chat or task name.

### A better specialist tool exists

Capabilities explains that Worktrunk could improve parallel switching or Entire could add commit-linked session evidence. The user sees supported Windows installation/configuration, data access, and a reversible integration diff. Nothing is installed just because a diagnostic rule recommended it.

---

## 22. Definition of done

The system is complete for v1 when one installation can give a developer a correct, bounded picture of registered projects, active/protected workspaces, current accepted decisions, scoped resource/credential references, and verified workstation findings—and can perform only the explicitly approved narrow actions it has been tested to perform.

It is not complete because it has many green badges, a graph visualization, a long feature list, or a successful build. Supported adapters, privacy boundaries, refusal/unknown behavior, real Windows proof, and recoverability must be documented.

The product should save the user from researching ten separate tools while remaining independent of those tools. The organizing abstraction is **project + evidence + explicit authority**, not an omniscient agent or a universal administrator.

**Frozen build direction:** one native local application; five bounded modules; a shared relational evidence model; strict separation of observation, authority, secrets, and actions; useful read-only release first; narrow certified capabilities added in order.


---

## 23. Primary-source register

Sources verified for this design on 18 September 2026. They support the vendor/platform facts, not a claim that Workstation has been implemented or certified. Adapter versions and provider commands must be revalidated at implementation and release.

**[S01] Rust Windows MSVC platform support** — Native Windows target support informs the Windows-first binary choice.

`https://doc.rust-lang.org/rustc/platform-support/windows-msvc.html`

**[S02] Microsoft windows-rs** — Official Rust bindings to Windows APIs.

`https://github.com/microsoft/windows-rs`

**[S03] SQLite write-ahead logging** — WAL coordination, locality, and concurrency constraints.

`https://www.sqlite.org/wal.html`

**[S04] SQLite release news** — Verify the WAL-reset fix and avoid the withdrawn 3.52.0 release.

`https://www.sqlite.org/news.html`

**[S05] rusqlite documentation** — Candidate Rust SQLite interface; resolve and test an explicit bundled version.

`https://docs.rs/rusqlite/latest/rusqlite/`

**[S06] Win32_Process** — Process creation/parent/identity fields; PID reuse requires birth identity.

`https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-process`

**[S07] Windows Job Objects** — Owned subprocess lifecycle and nested-job/breakaway caveats.

`https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects`

**[S08] Windows reparse points** — Reparse-aware collection and path-validation boundaries.

`https://learn.microsoft.com/en-us/windows/win32/fileio/reparse-points`

**[S09] CryptProtectData** — User/machine decryption scope and integrity; not isolation from same-user software.

`https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata`

**[S10] Windows process inheritance** — Child environment inheritance and explicit environment construction.

`https://learn.microsoft.com/en-us/windows/win32/procthread/inheritance`

**[S11] Docker contexts** — Daemon selection can be overridden; validate the actual local endpoint.

`https://docs.docker.com/engine/manage-resources/contexts/`

**[S12] WSL basic commands** — Global shutdown, targeted termination, and destructive unregister are distinct.

`https://learn.microsoft.com/en-us/windows/wsl/basic-commands`

**[S13] Worktrunk** — Specialist worktree integration; Windows git-wt naming and command side effects.

`https://github.com/max-sixty/worktrunk`

**[S14] Entire CLI** — Optional agent-session/checkpoint provenance source, not a required runtime.

`https://github.com/entireio/cli`

**[S15] Codbash** — Optional session navigation; do not duplicate a session dashboard.

`https://github.com/vakovalskii/codbash`

**[S16] Codex environment variables** — CODEX_HOME and documented location overrides; effective GUI state must still be discovered.

`https://learn.chatgpt.com/docs/config-file/environment-variables`

**[S17] Claude Code hooks** — Lifecycle integration; WorktreeCreate replaces creation rather than observing it.

`https://code.claude.com/docs/en/hooks`

**[S18] Git worktree** — Porcelain discovery, locking, pruning and removal semantics.

`https://git-scm.com/docs/git-worktree`

**[S19] Git status** — Machine-readable state and explicit handling of untracked/ignored content.

`https://git-scm.com/docs/git-status`

**[S20] MADR** — Lightweight structured architecture-decision record convention.

`https://adr.github.io/madr/`

**[S21] Backstage catalog descriptor format** — Service/resource ownership model used as a conceptual catalog reference.

`https://backstage.io/docs/features/software-catalog/descriptor-format/`

**[S22] Doppler CLI** — Provider CLI integration and credentialed environment execution.

`https://docs.doppler.com/docs/cli`

**[S23] 1Password op run** — Scoped subprocess secret-reference resolution and output behavior.

`https://www.1password.dev/cli/reference/commands/run`

**[S24] Infisical run** — Future provider execution adapter contract.

`https://infisical.com/docs/cli/commands/run`

**[S25] Bitwarden Secrets Manager CLI** — Provider reference/integration surface.

`https://bitwarden.com/help/secrets-manager-cli/`

**[S26] MCP server development** — Local stdio server protocol, SDKs, and output discipline.

`https://modelcontextprotocol.io/docs/2026-07-28/develop/build-server`

**[S27] MCP security best practices** — Trust boundaries and safe authorization handling; source version recorded explicitly.

`https://modelcontextprotocol.io/docs/2025-11-25/tutorials/security/security_best_practices`

**[S28] SQLite online backup API** — Consistent live-database backup instead of copying only the main database file.

`https://sqlite.org/backup.html`

**[S29] SQLite FTS5** — Local lexical search rather than mandatory vector infrastructure.

`https://www.sqlite.org/fts5.html`

