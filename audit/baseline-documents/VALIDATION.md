# Validation status — 0.4.0-alpha.1

**Implementation-only source delivery. No product/native certification was run.**

Executed: primary documentation research, source authoring and inspection, package inventory and
ZIP/member hash operations. These are not runtime validation. No claim of compilation, Rust tests,
SQL migration execution, native permission checks, actual secret encryption/rotation, agent handoff,
quota collection, repair success or Windows coexistence is made.

Authored tests and fixtures are included for the next approved phase. Their count is reported in
evidence/test-report.json; executed test counts remain zero. Old artifacts under evidence/history,
validation/ or verification/ belong to their named older source versions and are NOT current passes.

No Remote Desktop Commander call or Windows operation occurred. No source compile or test was
attempted. No Cargo.lock or executable was invented. The later certification build is gated by
scripts/build.ps1 -CertificationAuthorized and a matching manually dispatched CI permission.

Source inspection found and corrected several concrete implementation issues during authoring:
account cache isolation, destination-parent pinning, unbounded Git filter preflight moved into a
bounded worker, unknown contradiction handling, archive/backup coupling, old schema expectations,
quota-cost semantics, actual Copilot wire framing and configured-hook nonexclusive provenance.
Those corrections are SOURCE WORK, not proof that the resulting program is correct.

Read the package's limitations before running it. In particular, known writers must be coordinated
before a reviewed worktree removal; metadata fingerprints do not detect every concurrent edit;
DPAPI is not isolation from same-user software; tokens given to an approved child can be exfiltrated
by that child. External effects remain separate from read-only context and MCP.
