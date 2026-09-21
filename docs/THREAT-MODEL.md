> v0.4 update: current semantics and certification hold are in IMPLEMENTATION-V4.md,
> COMMANDS-V4.md and ../CURRENT-CAPABILITIES.md. The following section is preserved baseline context.

# Threat model and limitations

Trusted for this candidate: the logged-in operator, explicitly selected executable and
local data home, reviewed application source/dependencies, OS and SQLite. Untrusted:
repository text, imported summaries, resource candidates, quotas, model metrics and
external agent reports. Supported same-user malware isolation: **none**.

- Strict typed JSON rejects extra operation/authority fields. Preview binds to exact
  input bytes; approval is local intent, not proof a human rather than an agent invoked it.
- No raw transcripts/code are imported by default. Source paths and user-entered metadata
  are private. Context/report text is escaped; share projection creates count-only data.
- Secret references reject inline userinfo/query-token patterns. A few obvious secret
  patterns/canaries are rejected, not a comprehensive secret detector. Review arbitrary
  user-provided prose before sharing to a provider.
- DPAPI user-scope protects at rest, not same-user process access. Console input avoids
  ordinary echo/arguments, but no malicious endpoint/agent sandbox is claimed.
- The stdio MCP has fixed project/environment scope and read-only DB. It exposes no
  secret resolve, project override, arbitrary path, SQL, shell or assignment operation.
- Worktree metadata cannot prove source content equivalence or abandonment. Dirty,
  ignored and local-only work stays protected. No automatic deletion exists.
- Probe cancellation affects only owned workers; kernel stalls remain possible and
  return cleanup-unconfirmed. Native containment/reparse/ACL safety needs Windows tests.
- Resource discoveries are inert metadata. No candidate can self-declare review, install
  commands or privilege. Source review is not effectiveness/license certification.
- Cost data and benchmarks are explicit imports. License state is declarative input,
  not a legal entitlement verifier. No redistribution of vendor benchmark scores.
- Local metadata store corruption, size limits or failed backups produce errors; no
  default reset or C: fallback. Approved history is not silently pruned to make room.
