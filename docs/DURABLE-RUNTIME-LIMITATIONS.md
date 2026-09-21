# Durable runtime limitations

Workstation 0.5.0-alpha.1 is intentionally conservative.

- Windows 11 x64 is the primary runtime target. Ubuntu CI provides portability feedback, not production support. macOS is untested.
- The packaged Windows executable is unsigned. There is no installer or automatic updater.
- Offline protocol fixtures prove Workstation behavior, not current authenticated vendor behavior. Every live adapter/version/environment requires separate certification.
- Recovery never replays a prompt. If local and provider evidence cannot establish the exact run, the state remains reconciliation-required.
- There is no automatic retry, cross-agent routing, Work Item completion, or primary-assignment release.
- Event history is capped at 2,048 events per run and active durable storage at 512 runs. This alpha has no automated archival/compaction path.
- Cached watch is polling, bounded to 300 seconds and 256 emitted revisions.
- Cancellation is cooperative and can be delayed by an unresponsive owned process until its bounded grace handling completes.
- Verification checks exact content and registered commands, but the approved task itself is trusted code and is not a sandbox.
- Content hashing reads approved workspace files and can be expensive; it must be explicitly acknowledged.
- A malicious process running as the same user may race local files or interfere with processes. Administrators, compromised operating systems, toolchains, dependencies, and provider executables are outside the promise.
- Database backups are bounded and local. Operators remain responsible for protecting and testing recovery of their Workstation home.
- Historical evidence may describe older versions. Only `evidence/runtime-v5-build-report.json` is the v5 local build report.
