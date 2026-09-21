# Workstation Phase 1 implementation audit

**Baseline:** 0.4.0-alpha.1  
**Patched candidate:** 0.4.0-alpha.2  
**Disposition:** source fixes applied; non-native regressions exercised; **NOT Windows-certified, compiled or release-ready**.

This audit was performed on a separate container-side copy. The original 345-member ZIP and its 344 listed member hashes matched the delivered package. Its SHA-256 is `89fcfeaadfc1a749c91c2703e461fdcca7bc9b6aa393dbe7938cf7edb2e7ddb4`. The baseline was committed locally before changes. No Windows, Remote Desktop Commander, production checkout, live vendor, real credential or account was accessed.

## Executive result

The review identified **23 source defects/hardening gaps** and applied targeted corrections, including a reproduced data-loss path, an archive approval gap, incomplete-restore ambiguity, foreign-key validation omissions, and a vault write/read size mismatch. These are not cosmetic changes. Severity counts: **8 high, 14 medium, 1 low**. Severity represents impact if the affected path is exercised, not a CVSS assessment or proof of remote exploitability.

The project should remain an untrusted alpha source candidate. “93.27%” was a coarse scoped source-family tally; this audit does not increase, validate or convert it into a readiness percentage. Native compilation could expose additional defects.

## Evidence actually executed

| Suite | Passed | What it proves |
|---|---:|---|
| Audit: actual Git | 9 | Disposable Git behavior, hidden flags and preservation recipes; not the Rust supervisor |
| Audit: actual SQLite | 17 | All four shipped schemas, constraints, backup/WAL, exact archive-selection query |
| Audit: independent reference models | 16 | Boundary arithmetic, event ordering, size/identity behavior; not the Rust implementation |
| Audit: static source checks | 18 | Named guards/paths/policy and fixed interface surface exist; not compilation |
| **New audit suite** | **60** | Zero final failures/errors/skips |
| Inherited control suite on baseline | 116 | Historical Python SQL/Git/oracle/schema checks |
| Inherited control suite on patched tree | 116 | Same checks still pass; baseline rerun is not counted as additional coverage |

There are **176 distinct local checks passing on the final candidate** (60 new + 116 inherited), not 176 Rust or Windows tests. Current Rust test annotations are counted separately in delivery metadata; zero were executed. No cargo check/build/clippy/rustfmt, real Cargo.lock, Windows SDK/MSVC, DPAPI, Job Object, provider or handoff execution happened.

The first draft of the new harness had five static assertion errors (four guessed symbol/visibility names and a comment matched as code); those were fixed to target the actual production guards, not removed. Its original failure log is preserved as `evidence/phase1-tests-initial-harness.*`. It did not reveal five additional product failures. Final results and IDs are in `evidence/phase1-tests.json` and `evidence/patched-control-tests.json`.

## Most important reproduction: clean-looking worktree loses uncommitted content

Two isolated Linux Git fixtures set `--assume-unchanged` and `--skip-worktree` on a tracked file, then changed the file. Git status returned empty. **Non-force `git worktree remove` succeeded and deleted the unique uncommitted file** in each fixture. A preservation ref for HEAD cannot recover those uncommitted bytes.

The patch reads `git ls-files -v -z`, marks both hidden-state classes as blockers, validates sparse/operation metadata, and verifies the flag stream again before completing observation. Independent guarded recipes preserved the files. This is a confirmed Git-level defect in the prior decision logic, not a claim that the complete Rust removal action was executed. Native cleanup must still pass adversarial Windows tests.

Raw reproduction: `evidence/baseline-worktree-hidden-changes.json`. Git's documented flag semantics: source R1.

## Corrected findings

### A01 · HIGH · Hidden index flags allow destructive worktree misclassification

**Before:** A tracked file modified behind assume-unchanged or skip-worktree can yield an empty status. Non-force Git worktree removal still deletes it; preserving HEAD does not preserve that uncommitted content.

**Changed:** Capture the -v NUL index stream before/after observation, protect both flags, include flags in the metadata digest, and block sparse-checkout or unreadable operation markers.

**Evidence/limits:** Actual Git reproduction and independent guarded-recipe check; Rust capture/removal wrapper unrun. Same-user concurrent writes still require coordination.

**Files:** `crates/workstation-platform/src/control_workspace.rs`

### A02 · HIGH · ACP path guard rejects normal Windows roots but under-validates nonexistent file names

**Before:** Canonicalizing a Windows root creates a verbatim path that the local path validator rejects. A nonexistent target was checked through its parent without first validating the complete target spelling (including ADS/ambiguous names).

**Changed:** Keep a validated normal root spelling, lexically validate the whole requested path, distinguish true NotFound from other errors, reject dangling links and protected configuration directories.

**Evidence/limits:** Source trace plus Rust canonicalize documentation. Native Windows path, ADS and provider callback tests are authored, not run. This is policy, not filesystem isolation.

**Files:** `crates/workstation-platform/src/rpc.rs`

### A03 · MEDIUM · Codex completion notification can be lost before the turn/start reply

**Before:** The RPC request loop discarded a terminal turn notification arriving before the request response. The subsequent waiter could wait until timeout for an event already received.

**Changed:** Retain only bounded terminal thread/turn/status metadata, consume it in the turn waiter, and cap the pending terminal queue at 32.

**Evidence/limits:** Control-flow review and independent event-order reference; real Codex/Rust transport unrun.

**Files:** `crates/workstation-platform/src/rpc.rs`

### A04 · MEDIUM · Known unsupported Gemini read-only continuation fails after starting a session

**Before:** The pre-existing unsupported read-only guard occurred only after connecting, authenticating and creating/loading a vendor session.

**Changed:** Reject this known unsupported combination before connect or session creation.

**Evidence/limits:** Static call-order verification. Does not certify another provider mode as a sandbox.

**Files:** `crates/workstation-platform/src/acp_driver.rs`

### A05 · HIGH · Quarantine undo and absence checks could conflate lookup failures with absence

**Before:** Path.exists/is_ok patterns hide errors or dangling entries; restore also lacked full post-rename verification.

**Changed:** Introduce strict entry_exists/require_absent, validate the original path, and recheck restored identity/content plus source disappearance. Preserve uncertainty after partial effects.

**Evidence/limits:** Source checks and Linux filesystem semantics; no real quarantine/Windows socket action executed. Rename races are not claimed eliminated.

**Files:** `crates/workstation-platform/src/paths.rs`, `crates/workstation-platform/src/repairs.rs`, `crates/workstation-platform/src/engine.rs`

### A06 · HIGH · Archive approval did not bind exact selected records

**Before:** The approved archive operation contained only cutoff and count. New qualifying records or payload changes could change what execution archived after approval.

**Changed:** Add an exact bounded selection digest to the operation, deterministic tie-breaking and execution-time re-selection before any archive write. Change the effects policy ID to require replanning.

**Evidence/limits:** Exact shipped SQL selection executed with Python SQLite; Rust plan digest/executor unrun.

**Files:** `crates/workstation-core/src/operations.rs`, `crates/workstation-core/src/effects.rs`, `crates/workstation-platform/src/journal_archive.rs`, `crates/workstation-platform/src/operation_runtime.rs`

### A07 · MEDIUM · Archive builds oversized payload collections before enforcing its byte budget

**Before:** Record count was checked too late (including an off-by-one), and the aggregate byte ceiling was enforced only after retaining/serializing the full pack.

**Changed:** Enforce aggregate encoded-entry and record limits before retaining each entry, reserve header space, use checked arithmetic, and verify hydrated pack size/project binding.

**Evidence/limits:** Independent bounded reference plus static wiring; no native memory benchmark or archive executor run.

**Files:** `crates/workstation-platform/src/journal_archive.rs`

### A08 · HIGH · Failed restore can leave an apparently usable partial home

**Before:** After copying the database, a failure restoring required archive dependencies left a normal-looking initialized home; future opens did not know restore was unfinished.

**Changed:** Create and sync restore.pending before data copy, reject normal and read-only opens while present, and remove it only after dependencies and integrity checks succeed.

**Evidence/limits:** Marker/error-path source audit and reference fixture; crash-injection against the Rust restore is still required.

**Files:** `crates/workstation-platform/src/storage.rs`

### A09 · HIGH · Backup/restore structural checks omit foreign-key integrity

**Before:** SQLite integrity_check/quick_check can return ok for broken foreign keys. The prior backup/restore health checks could accept internally inconsistent project/resource relationships.

**Changed:** Run PRAGMA foreign_key_check during owned DB health, backup validation and before restoring a source.

**Evidence/limits:** Actual SQLite reproduction with shipped schemas; Rust wrapper and Windows backup unrun.

**Files:** `crates/workstation-platform/src/storage.rs`

### A10 · HIGH · Vault can write an accepted secret that its reader rejects

**Before:** A 16 KiB byte value serialized as decimal JSON can exceed 64 KiB before DPAPI overhead, but the ciphertext reader was capped at 64 KiB.

**Changed:** Share a 16 KiB plaintext and 128 KiB ciphertext limit across legacy/versioned paths; reject oversized ciphertext before writing and invalid plaintext after unprotect.

**Evidence/limits:** Actual serialization-size reference using a valid UTF-8 secret. No DPAPI operation executed; native maximum-size roundtrip tests authored.

**Files:** `crates/workstation-platform/src/vault.rs`, `crates/workstation-platform/src/secret_lifecycle.rs`

### A11 · HIGH · Windows secret-lifecycle branch references Path without importing it

**Before:** The Windows-only implementation names Path::new without std::path::Path in scope, a direct source-level name-resolution defect.

**Changed:** Add the cfg(windows) Path import alongside Read/Write.

**Evidence/limits:** Static import/name audit, not an executed compiler diagnostic. The full tree may still contain other compilation defects.

**Files:** `crates/workstation-platform/src/secret_lifecycle.rs`

### A12 · MEDIUM · Plan-fit utilization can combine different accounts

**Before:** Completed cycles had provider and bucket but no explicit account identity, so apparently comparable cycles from different subscriptions could be aggregated.

**Changed:** Add optional account_alias to the wire shape; missing identity produces unknown, mixed accounts reject, and complete-cycle keys include account.

**Evidence/limits:** Independent identity references and source assertions. Old imports are still parseable, but now need explicit account identity for a plan-fit verdict.

**Files:** `crates/workstation-core/src/economics.rs`, `fixtures/control/plan-cycles.json`

### A13 · MEDIUM · Finite economic inputs can produce nonfinite derived results

**Before:** Tiny positive denominators and large even-median sums can create infinity; some derived outputs could then be serialized as null or reported without an explicit numeric boundary.

**Changed:** Use finite-ratio guards, overflow-safe median arithmetic, reject impossible scenario rates and withhold unsupported ratios.

**Evidence/limits:** Independent IEEE-754 reference checks; no claim of executed Rust math or calibrated forecast confidence.

**Files:** `crates/workstation-core/src/economics.rs`

### A14 · MEDIUM · Optional context sections can prevent all project context

**Before:** A failing discovery/economics/cache/timeline helper propagated its error, hiding essential decisions/resources/Work Items too.

**Changed:** Collect optional sections as bounded success or typed failure metadata; preserve essential authoritative lookups as fail-closed.

**Evidence/limits:** Static flow review; authored Rust optional-section tests remain unrun.

**Files:** `crates/workstation-platform/src/control_store.rs`

### A15 · MEDIUM · Reviewed candidate count can exceed the discovery matcher budget

**Before:** The query allowed 77 imported entries on top of 27 builtins (104) while the matcher rejects more than 100, potentially disabling discovery for a growing catalog.

**Changed:** Enforce builtin-aware capacity during review and query, retain excess existing records rather than deleting them, and report omitted capacity.

**Evidence/limits:** Packaged catalog counted plus source capacity review; actual Rust review/import transaction unrun.

**Files:** `crates/workstation-platform/src/control_store.rs`

### A16 · MEDIUM · Empty strings can masquerade as trial evidence or reviewed resource metadata

**Before:** Option presence was accepted even for blank evidence/revision strings, allowing a claimed usefulness outcome without a meaningful reference.

**Changed:** Reject blank or oversized evidence/revision and empty reviewed resource fields; validate equivalence and feedback times.

**Evidence/limits:** Input validation source fix; references still need genuine operator evaluation, not automated truth inference.

**Files:** `crates/workstation-core/src/discovery.rs`

### A17 · MEDIUM · Collision detection performs repeated all-pairs path normalization

**Before:** Large accepted actor/path inputs caused an actors-squared times paths-squared loop with repeated allocation.

**Changed:** Normalize once into bounded sets; use exact, ancestor and indexed-descendant checks; limit aggregate path bytes and validate actor roles/times.

**Evidence/limits:** Independent reference equivalence over 2,048 path/set comparisons; not a Rust/Windows performance benchmark.

**Files:** `crates/workstation-core/src/workspace_policy.rs`

### A18 · MEDIUM · Ownership correlation repeatedly scans bindings and accepts ambiguous prior identities

**Before:** Each ancestry step re-scanned the binding list; duplicate prior process keys and ill-ordered evidence times were insufficiently rejected.

**Changed:** Pre-index fresh bindings, cap ended-session evidence, validate binding time ordering, and reject duplicate previous process identities.

**Evidence/limits:** Source/dataflow inspection and authored Rust regression; runtime matching untested. All processes remain protected.

**Files:** `crates/workstation-core/src/ownership.rs`

### A19 · MEDIUM · Process graph can claim complete coverage with unreadable fields

**Before:** Unreadable executable or resource fields did not always reduce coverage; resource observations used a second handle without an explicit birth consistency check.

**Changed:** Track field readability, reduce per-process and graph coverage, and compare handle creation time before merging resource observations.

**Evidence/limits:** Native source review only. A PID-reuse exploit is not claimed reproduced; Windows collection must still be exercised.

**Files:** `crates/workstation-platform/src/process_graph.rs`

### A20 · MEDIUM · Malformed or future timestamps can overflow arithmetic or appear fresh

**Before:** Signal time subtraction accepted negative extreme values, and a future last-observed timestamp could produce recently-reported ownership.

**Changed:** Validate signal, checkpoint and lease timestamps before subtraction; exact expiry/future observations remain uncertain.

**Evidence/limits:** Source reasoning/reference boundary checks; native execution and clock-jump integration deferred.

**Files:** `crates/workstation-core/src/health_rules.rs`, `crates/workstation-core/src/continuity.rs`

### A21 · MEDIUM · Pinned time dependency is in a published affected version range

**Before:** time=0.3.44 falls in RUSTSEC-2026-0009 affected versions; the advisory concerns RFC2822 recursion/stack exhaustion.

**Changed:** Move the exact direct pin to 0.3.47, the advisory patch floor; preserve features and record the unresolved transitive audit.

**Evidence/limits:** Primary advisory checked. Current code uses RFC3339; exploit reachability is NOT established. No Cargo resolution or advisory scan of a real lockfile ran.

**Files:** `Cargo.toml`

### A22 · MEDIUM · Some supposedly bounded config reads can block on special files or allocate without limit

**Before:** Worktrunk empty-config verification used an unbounded fs::read; config/database opens did not consistently reject non-regular files before open.

**Changed:** Add regular-file prechecks, exact-size bounded Worktrunk config reading and strict project-config presence checks.

**Evidence/limits:** Linux special-file semantics and source checks. Path replacement races against malicious same-user writers remain outside this guard.

**Files:** `crates/workstation-platform/src/paths.rs`, `crates/workstation-platform/src/storage.rs`, `crates/workstation-platform/src/specialists.rs`

### A23 · LOW · Read-only report says no repairs are implemented anywhere

**Before:** The inherited report wording described the entire product as lacking cleanup/repair despite added explicit operation paths.

**Changed:** Clarify that this particular read-only assessment performed no cleanup or repairs.

**Evidence/limits:** Documentation/output consistency correction only; not a change in authority or capability.

**Files:** `crates/workstation-core/src/render.rs`

## Full audit coverage and retained boundaries

All seven modules and the shared effects/storage/protocol layer were included in source review. The domain matrix below distinguishes actual evidence from remaining gates. It is not a claim that every path or vendor combination was tested.

| Audit area | Reviewed scope | Evidence and limitations |
|---|---|---|
| Compile/static risk | Manifests, cfg paths, imports, module/include references, unwrap/unsafe surfaces | Name-resolution fix A11; structural checks only. Compiler/linter still unavailable. |
| Dependencies | All direct pins/features and their usage; targeted advisory verification | A21 patched direct pin; no complete resolved graph, lockfile, audit or SBOM. O05 remains open. |
| Three-crate boundaries | Core/platform/CLI effects direction and MCP interface | Core does not gain process/network/DB effects; no new crate or service. |
| SQLite/data model | All four schema migrations, keys, immutability/CAS and temporal query | A09; actual Python SQLite tests including schema4, one-shot runs, keys, leases and secret CAS. |
| Migrations/restore | Version1/2/3/4 upgrade/backup/restore paths and archive dependencies | A08/A09; SQL rollback/backup exercised; Rust restore/crash tests deferred. |
| Workspaces | Capture, hidden flags, local-only/ignored work, operation markers and cleanup eligibility | A01; real destructive Git edge case reproduced twice, fixed guard recipe protects it. |
| Cleanup/repairs | Typed plans, exact targets, expiry, quarantine, undo, preservation | A05/A06; no live external repair; time-of-check/use races O03 remain. |
| Docker | Local named-pipe target, data-root exclusion, stopped-state quarantine and bounded startup | No broadened recipe or WSL shutdown; source review only, actual sockets deferred. |
| Codex recovery | Declared home, error freshness, invalid JSON, UUID payload preservation | Existing recipe remains narrow; valid-but-stale registry recovery is not added. |
| Process ownership | Birth identity, indexed bindings, prior samples and coverage | A18/A19/A20; no process termination authority. Windows graph unrun. |
| Chronicle | Current/effective/known time, immutable decisions and lifecycle provenance | Temporal SQL exercised; A20 closes timestamp arithmetic/lease errors; no autoacceptance. |
| Atlas/environments | Exact environment references, manifests, freshness and provider claims | Cross-environment SQL constraints exercised; HTTP success remains not ownership proof. |
| Vault/secrets | DPAPI sizes, binding, rotation/CAS, provider/task output handling | A10/A11/A22; serialization boundary demonstrated; DPAPI and provider operations unrun. |
| Capabilities | Matching budget, scoped feedback, catalog review/import and optional context | A14/A15/A16; no autoactivation or runtime Python dependency. |
| Economics | Meter/account identity, reset boundaries, finite arithmetic, model/scenario ratios | A12/A13; 28 inherited Python oracle tests plus new reference boundaries; real quotas absent. |
| Continuity | Checkpoint invariants, leases, Codex/ACP/Copilot event handling | A02/A03/A04/A20; no real agents/resume/handoff performed. |
| MCP | 13 fixed read-only tools, parameter scope, line/result bounds and error output | Static contract retained; native stdio fuzz/runtime tests not run. |
| CLI | Explicit home, command wiring, JSON sizes, static report behavior | A14/A23; no native --help/exit-code smoke evidence. |
| Error handling | Absent vs denied, uncertainty after effects, bounded sanitized failures | A05/A08/A14/A22. No raw credential output added. |
| Panics/unsafe | Occurrence inventory, checked time/size arithmetic, OS boundary handles | A11/A13/A20; no Miri, Rust compiler, sanitizer or native crash evidence. |
| Concurrency | Primary uniqueness, CAS heads, immutable exact approval and archive selection | A06; actual SQL primary/secret/one-shot behavior exercised; OS effect race still open. |
| Own storage | Observation budgets, archive aggregate memory, backup dependency validation | A07/A15/A17; no physical shrink or full peak-memory claim. |
| Privacy | Allowlist share output, metadata projections, candidate content and secret boundaries | A03 minimizes queued event content; no transcripts/accounts/cookies read. |
| Docs vs code | Current docs vs baseline, misleading report and new input/policy changes | A23; current audit overrides old milestone/test claims; baseline retained as history. |
| Packaging | Original ZIP/manifest, changed-files diff, new member hashes and exclusions | Rebuilt source-only ZIP; no executable/lockfile, vault blobs, test homes or .git directory. |

## Unresolved findings / mandatory later gates

### O01 · BLOCKING · Rust compilation, formatting and lint remain unexecuted

Resolve the genuine Cargo graph, run rustfmt/check/Clippy/tests on both targets after approval. Static source inspection is not parsing/type checking.

### O02 · HIGH · Windows filesystem/process/DPAPI behavior remains unverified

Test ACLs, reparse points, Job Object teardown, Windows canonical path variants, real special sockets, low disk and restore crash boundaries. No automatic cleanup should be trusted before this gate.

### O03 · HIGH · Path and executable identity checks are not atomic use operations

Same-user races between hash/identity checking and open/spawn/rename are not eliminated. Stronger handle-relative operations or a narrower threat model must be accepted; no OS isolation/exclusive-worktree claim.

### O04 · HIGH · Live vendor protocol/authentication/permission semantics remain unproved

Exercise exact installed Codex/Claude/ACP/Copilot versions, callbacks, failed launch and partial completion. Public schemas and fabricated fixtures cannot certify vendor behavior.

### O05 · HIGH · Unresolved dependency graph and transitive rustls advisory

No Cargo.lock exists. RUSTSEC-2026-0285 affects rustls >=0.23.13,<0.23.45. Determine actual linkage and audit the resolved graph. Do not assert this package is affected or fixed without resolution.

### O06 · MEDIUM · Secret protection is scoped, not end-to-end memory isolation

DPAPI protects at rest. Child programs can disclose injected secrets, same-user code can access them, and all intermediate copies are not guaranteed zeroized. Native secret canary/maximum-size/rotation/recovery tests remain mandatory.

### O07 · MEDIUM · Metadata freshness is not byte-identical content proof

Same-size edits with restored modification time can evade the existing handoff metadata digest. Receiving agents must independently inspect content/tests; stronger optional content evidence remains a design decision.

### O08 · MEDIUM · Failure injection and performance targets remain unmeasured

Interrupt real archive, repair, secret rotation and restoration after each effect step; test concurrent claims, huge input and low disk. No p95 latency or absolute recovery guarantee is claimed.

### O09 · MEDIUM · Code compression reduces human review reliability

Much of the inherited Rust is minified into long lines. Run a real rustfmt pass and review formatted diffs before certification; do not replace that with an improvised formatter.

## What intentionally did not change

Seven modules; three Rust crates; schema4; no cloud service or background daemon; 13 read-only project-scoped MCP tools; no arbitrary MCP shell/SQL/filesystem/secret methods. Unknown or expired evidence does not authorize cleanup. No automatic agent killing, global WSL shutdown, provider switching, remote credential revocation or source-transcript collection was introduced.

Real third-party sessions/accounts remain unexercised. Catalog descriptions remain reference material, not license/safety/effectiveness certifications. Handoff metadata is not a byte-identical source snapshot. Existing working Windows recovery scripts were not edited.

## Native handoff (paused pending permission)

1. Inspect this exact source and patch; generate/review a genuine dependency lock and advisory report.
2. Run rustfmt, compiler/Clippy and all Rust tests without weakening assertions. The new native tests must run, not merely be counted.
3. Exercise Windows path/ACL/ADS/reparse, process birth, Job containment and DPAPI canary/size/rotation cases in disposable homes.
4. Exercise backup/restore/archive failure points, exact plan invalidation and hidden Git flags before enabling destructive operations.
5. Test exact authenticated agent/provider versions, permission callbacks and real cross-agent continuation only with the user's approval.

`COMPATIBILITY.md` describes changed input expectations and mandatory replanning. `open-items.json` is the unresolved queue. `source.patch` is the exact implementation delta. `findings.json` binds corrections to files and regression evidence. The source ZIP is a handoff, **not an installable/certified Windows application**.
