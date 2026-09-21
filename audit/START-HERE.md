# Read this audit candidate first

Version **0.4.0-alpha.2**, derived from the exact delivered 0.4.0-alpha.1 archive.

1. `AUDIT.md`: 23 source corrections, 25 review areas, and actual evidence limits.
2. `COMPATIBILITY.md`: changed effect policy, stricter worktree blockers and account identity.
3. `open-items.json`: nine unresolved gates, including native compile/platform/protocol tests.
4. `source.patch`: exact code/config/fixture changes (27 files), independently applied to a
   disposable baseline extraction and checked against candidate file hashes.
5. `evidence/phase1-tests.json` and `evidence/patched-control-tests.json`: 60+116 actual local
   checks. They are not compiled-Rust results.

The full patched source is in this ZIP. Original source is not overwritten. No user Windows
device, account, real credential or production workspace was touched. Wait for explicit user
green light before native build, Windows testing or executing the changed operation paths.

Do not use old effect approval digests: replan under workstation.effects.v4.audit1. Do not
clear hidden Git flags or remove restore.pending merely to force an operation through.
