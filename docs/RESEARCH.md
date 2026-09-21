# Implementation research — 18 September 2026

These primary sources informed this slice. They support platform/API choices, not a claim that the source has been compiled or tested on Windows. Versioned documentation/search caches can differ; native Cargo resolution and compilation remain mandatory.

| Source | Implementation consequence |
|---|---|
| Rust Windows MSVC platform support | Target Windows 11 x64/MSVC and build natively with the documented C++ prerequisites. |
| Rust `Child` documentation | Explicit child wait/reaping; dropping a handle is not lifecycle cleanup. |
| Microsoft Job Objects | Contain our own stdin-gated collectors and their descendants; never attach existing agents. Test nested-job behavior on Windows. |
| Microsoft Win32_Process | A parent PID/name is not safe ownership proof; keep process creation timestamps and mark ownership unknown. |
| Microsoft GetProcessMemoryInfo | Distinguish working set from private commitment; denied fields remain unknown. |
| Microsoft reparse-point documentation | Do not blindly traverse junctions, symbolic links or placeholders. |
| Microsoft security descriptor/SID documentation | Restricted own-home ACL using owner/system/admin principals; does not isolate same-user code. |
| Git worktree documentation | Use NUL-terminated porcelain registration output; a worktree record is not permission to remove files. |
| Git environment documentation | Suppress optional locks, prompts, pager, global/system config and lazy fetch for inspection. |
| SQLite WAL / backup documentation | Own local database only; bounded writes and proper backup API instead of copying a live database without WAL. |
| SQLite release documentation | Runtime gate rejects pre-fix/withdrawn engines. Record actual bundled engine rather than assuming a wrapper version proves it. |
| rusqlite backup documentation | Bounded `step` loop explicitly accounts for `More`, `Busy`, `Locked`, and `Done`. |

## Primary references

- Rust MSVC: https://doc.rust-lang.org/rustc/platform-support/windows-msvc.html
- Rust pinned standard library / child process: https://doc.rust-lang.org/std/process/struct.Child.html
- Microsoft Windows Rust bindings: https://github.com/microsoft/windows-rs
- Job Objects: https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects
- Process identity: https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-process
- Process memory: https://learn.microsoft.com/en-us/windows/win32/api/psapi/nf-psapi-getprocessmemoryinfo
- Reparse points: https://learn.microsoft.com/en-us/windows/win32/fileio/reparse-points
- SID strings: https://learn.microsoft.com/en-us/windows/win32/secauthz/sid-strings
- Git worktree: https://git-scm.com/docs/git-worktree
- Git environment: https://git-scm.com/docs/git
- SQLite WAL: https://www.sqlite.org/wal.html
- SQLite backup: https://sqlite.org/backup.html
- SQLite release news: https://www.sqlite.org/news.html
- SQLite 3.53.2 release: https://sqlite.org/releaselog/3_53_2.html
- rusqlite: https://docs.rs/rusqlite/latest/rusqlite/
- rusqlite backup outcomes: https://docs.rs/rusqlite/latest/rusqlite/backup/enum.StepResult.html

## Dependency selection

The source pins direct dependencies and Rust 1.97.1 based on the available documentation. This is not a claim that every selected version is the latest possible release. The intended bundled SQLite for rusqlite 0.40.2 is 3.53.2; startup checks actual linked version/source ID. SQLite's release register also lists newer maintenance releases, so the publishing maintainer must review advisories and lockfile resolution before approving a native release.

No dependency lock/checksum set is fabricated. Complete Cargo resolution is blocked in the authoring environment and must occur before the first native package. Compiler, API and dependency compatibility have not been established by these web pages.

## How the research changed the build

A simple recursive loop is not a reliable timeout around blocking filesystem I/O; the implementation isolates our collectors. A process count is not an orphan detector; the implementation protects every process. A worktree list is not a cleanup safety assessment; the implementation does not inspect dirty/ignored files yet and offers no removal. Reading a cached health snapshot is not a fresh health check; the report explicitly says so.

Public bug reports and the user's transcript seed synthetic incident-policy fixtures only. No version-specific live repair is shipped on the strength of those reports.
