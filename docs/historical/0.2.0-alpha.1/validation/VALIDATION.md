# Validation record

**Delivery: source candidate, not a certified native application.**

## Executed here

- **20/20 independent artifact/reference checks passed**, zero failures/errors/skips.
- JSON Schema syntax, valid examples, negative disclosure/action contracts.
- Cargo manifest parsing, explicit direct pins and three-crate structure.
- Actual SQLite schema execution, constraint rejection, twenty-scan retention query and backup/restore using Python's SQLite binding.
- Actual read-only Git worktree recipe on temporary Linux repositories containing Unicode paths, a dirty tracked file, an untracked file and an ignored secret canary; hashes remained unchanged and the canary was not emitted.
- Static checks for excluded command families.

Raw evidence: `artifact-checks.json`, `artifact-checks.log`.

These tests ran against artifacts/reference recipes. **They did not execute the Rust application.** Host Python linked SQLite 3.46.1; it is not the intended bundled SQLite runtime.

## Authored, not run

- **46 Rust tests** enumerated in `rust-tests-authored.json`.
- Unit cases for path policy, missing home, scanner bounds, unsafe classifications, Unicode/escaping, Git parsing, SQLite initialization/backup and schema gates.
- Binary integration cases for no-save/cached results, canary isolation, share projection, timeout/flood, and Windows child containment.
- Windows PowerShell smoke and GitHub Actions workflow.

## Blocked, not passed

The Linux authoring environment contains no rustc/cargo toolchain. Network/package-server retrieval attempts failed. Consequently there is **no real Cargo.lock, no compiled executable, no native API validation, no Rust test result, no Windows performance measurement and no code signing** in this archive.

The Windows build script explicitly resolves the initial lockfile only with operator approval and stops on any compiler/lint/test failure. Native compilation and test failures may reveal source defects; fix them and rerun, do not disable the gate.

## First native acceptance priorities

1. Exact dependency resolution/compiler compatibility.
2. Home ACL creation/access, fixed NTFS/reparse checks, Unicode paths.
3. Worker timeout/output limits and no surviving owned descendants.
4. Actual linked SQLite patch/source ID; live backup/integrity and contention.
5. No changes to registered application files/repositories.
6. Missing/disconnected drive produces no fallback data on C:.
7. Private report and share projection canary tests.
8. First read-only coexistence with active agents. No automatic repairs or cleanup.

**Full FS-1.0 R0 gate: NOT PASSED.** The implementation is a testable source slice awaiting native build evidence, not an assertion of production readiness.

## D1 amendment (source package revision d1)

Contextual Resource Discovery is now accepted in scope and staged for R4/R5.
See D1-VALIDATION.md for **44 executed specification/reference tests** and preservation of
41 original runtime/build/contract artifacts. Those results are not Rust, Windows or discovery
product passes. The 28 added product cases remain not run.
