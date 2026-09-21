# Security policy

Workstation 0.5 is a public alpha. It should be evaluated as a local developer tool, not deployed as a privileged, unattended repair service.

## Reporting a vulnerability

Use GitHub private vulnerability reporting if it is enabled for the public repository. If it is not enabled, ask the repository owner for a private reporting channel without including exploit details. Do not disclose credentials, provider transcripts, prompt bodies, Workstation databases, private reports, or machine-identifying paths in a public issue.

No security contact address is fabricated in this source tree. The publishing maintainer must enable a private channel before announcing the repository.

## Supported version

Only the newest published `0.5.0-alpha.*` revision is intended to receive security fixes during the alpha. Older source snapshots are historical evidence, not supported releases.

## Security boundaries

- Workstation runs as the current user and stores private local metadata under an explicitly selected home.
- Credential values are not accepted in integration profiles or emitted in release evidence. Provider authentication remains provider-owned.
- Durable status and events retain bounded state metadata, not prompt bodies or arbitrary provider logs.
- Cached read commands do not contact providers. Provider reconciliation requires a specific approval digest.
- Child process control is limited to a birth-qualified process tree created for the approved run. Unknown ownership remains protected.
- Verification uses content hashes and pre-registered task IDs; file timestamps alone are not evidence.
- Work Item completion and primary-assignment release are never inferred from provider completion or a passing verification.
- Same-user malicious code, administrators, compromised dependencies/toolchains, kernel failures, and hostile provider binaries are outside the security promise.
- The portable ZIP is unsigned. Verify the adjacent SHA-256 file before use.
