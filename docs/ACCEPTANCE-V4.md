# v4 acceptance additions — authored, not executed

These 50 cases supplement existing tests; they are not substitutions or passing evidence.
Run only after the user authorizes certification. Keep failures and unsupported interfaces visible.

| ID | Area | Required result |
|---|---|---|
| V4-001 | identity | Reject reused PID with changed creation time; do not bind stale parent lineage. |
| V4-002 | identity | An observed executable-file digest is not loaded-image provenance; unknown cannot authorize actions. |
| V4-003 | identity | Managed protocol session binding and configured hook ancestry agree on project/session scope. |
| V4-004 | identity | Ambiguous/shared hook ancestors remain nonexclusive and protected. |
| V4-005 | identity | Incomplete global process coverage cannot prove an orphan or disposal safety. |
| V4-006 | health | Required or contradictory evidence unknown produces insufficient evidence, not confirmed root cause. |
| V4-007 | health | Stale health/profile data and changed installation digest invalidate old correlations. |
| V4-008 | health | Unsupported version/protocol returns an explicit unsupported result without fallback shell execution. |
| V4-009 | workspace | Dirty tracked, untracked or ignored content blocks cleanup even with manual quiescence. |
| V4-010 | workspace | Local-only commits, work in progress, submodules and missing coverage remain protected. |
| V4-011 | workspace | Expired/unreleased primary session is uncertain and blocks cleanup. |
| V4-012 | workspace | Main worktree, bare repository and locked worktree removal are refused. |
| V4-013 | workspace | Work Item/worktree/branch/file overlap is reported without killing or switching owners. |
| V4-014 | workspace | Changed HEAD/status/path identity after preview invalidates removal. |
| V4-015 | workspace | Preservation ref is created only with zero-old/no-overwrite semantics and survives failures. |
| V4-016 | workspace | Nonforce removal cannot delete a branch or invoke broad prune. |
| V4-017 | workspace | Destination parent identity replacement, broken link and access denied never mean empty. |
| V4-018 | workspace | Worktrunk uses exact empty config and declines remote CI/summary collection or project hook execution. |
| V4-019 | workspace | Created worktree is verified against requested repository/HEAD/branch; partial creation not automatically destroyed. |
| V4-020 | chronicle | Configured lifecycle events are scoped and idempotent where applicable; turn-end does not end a session. |
| V4-021 | chronicle | Same external session ID in different project/vendor cannot merge history or authority. |
| V4-022 | chronicle | Hook-reported decisions are proposals, never silently accepted. |
| V4-023 | atlas | Local credential rotation failure before/after file write preserves existing head or records an orphan encrypted file. |
| V4-024 | atlas | Compare-and-swap rejects concurrent/stale secret-version transitions. |
| V4-025 | atlas | Revoked head cannot fall through to a legacy ciphertext resolver. |
| V4-026 | atlas | Credential generation change invalidates pending effects in the affected environment. |
| V4-027 | atlas | Wrong user/machine/tampered DPAPI value fails; recovery status distinguishes metadata from decryptability. |
| V4-028 | atlas | Reports, MCP, plans, arguments, journal errors and share output contain no injected secret canary. |
| V4-029 | capabilities | Existing availability is based on fresh scoped profile evidence, not mere catalog membership. |
| V4-030 | capabilities | Adoption produces separate resource/decision proposals without applying or accepting them. |
| V4-031 | capabilities | Revision changes invalidate applicability; saved/dismissed feedback stays appropriately scoped. |
| V4-032 | economics | Copilot unknown/unlimited quota and missing/reset-changing timestamp do not produce a fabricated runway. |
| V4-033 | economics | Account-isolated caches cannot mix two integrations with the same provider or label. |
| V4-034 | economics | Claude missing statusline window stays unknown; cost remains estimated not subscription debit. |
| V4-035 | economics | Gemini cumulative session tokens never appear as remaining subscription or context capacity. |
| V4-036 | economics | Bad/negative/nonfinite numeric fields are rejected without recording raw provider text. |
| V4-037 | protocol | Copilot Content-Length rejects duplicate/oversized/truncated frames and noncorrelated results. |
| V4-038 | protocol | Unexpected protocol revision or permission/options response rejects continuation before prompt. |
| V4-039 | protocol | Configured providers cannot invoke arbitrary host tools; unadvertised ACP load/auth/mode remains unsupported. |
| V4-040 | protocol | Resume binds the exact project/workspace identity and does not displace an existing primary. |
| V4-041 | protocol | Failure/timeout preserves uncertain ownership and does not declare tests or Work Item done. |
| V4-042 | archive | Only old terminal records are archived; running/indeterminate entries stay in active storage. |
| V4-043 | archive | Crash before/after each archive write/transaction is recoverable without losing logical history. |
| V4-044 | archive | Tampered/missing archive pack or entry digest fails hydration rather than hiding data loss. |
| V4-045 | archive | Backup database and archive companions restore together into a new home only. |
| V4-046 | archive | Archive file/count limits stop optional archival; approved records and required backups are preserved. |
| V4-047 | shared | Fresh install, v1/v2/v3 to v4 migration, failure rollback and source manifest remain coherent. |
| V4-048 | shared | 13 read-only MCP tools and cached context perform no external effects or unexpected scans. |
| V4-049 | shared | Windows lock/job/pipe teardown, Unicode/reparse/ACL coexistence and no C fallback pass on actual host. |
| V4-050 | release | Authenticated real multi-agent task and quota exercises are explicitly authorized and independently reported. |

Rust test source: crates/workstation-core/tests/implementation_v4.rs and
crates/workstation-platform/tests/implementation_v4.rs, plus module-local tests. Native effect,
account, transport and crash-injection tests still require environment-specific fixtures.
