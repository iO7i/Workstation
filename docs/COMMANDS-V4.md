# v4 command additions — not executed in this delivery

Read the baseline command documentation for init/project/record/work/checkpoint/context. All source
is uncompiled. The commands below describe the intended future interface after a separate build gate.
No command shown here is permission to run it on the user's machine now.

## Observations and cached views

```
workstation --home D:\Workstation health scan --project <id>
workstation --home D:\Workstation health latest --project <id>
workstation --home D:\Workstation health rules
workstation --home D:\Workstation workspace scan --project <id>
workstation --home D:\Workstation workspace graph --project <id>
workstation --home D:\Workstation workspace eligibility --project <id> --workspace <id>
workstation --home D:\Workstation discovery inventory --project <id>
workstation --home D:\Workstation atlas status --project <id> --environment <id>
workstation --home D:\Workstation telemetry summary --project <id>
workstation --home D:\Workstation journal inventory --project <id>
```

Scan means local metadata workers, not remote provider probing. Latest/context/MCP are cached.
Eligibility returns blockers and no delete permission. `discover` (legacy root candidates) is a
separate command from the new `discovery inventory`.

## Explicit typed plans

Inert synthetic authoring examples are in `examples/operations/`. Review real IDs locally.

```
workstation --home D:\Workstation effect prepare --project <id> --environment <id> --input <request.json>
workstation --home D:\Workstation effect show --id <plan-id>
```

After later native certification and approval, the existing `effect apply` requires the exact
plan digest and the explicit prerelease-execution acknowledgement. It is not printed here as
an unattended recipe. Never batch arbitrary approved actions or reuse stale plan IDs.

Requests now include workspace cleanup/create, secret rotate/revoke and journal archive. New
query kinds include Copilot quota/models, vendor session lists, protocol health and Worktrunk list.

## Protection and discovery drafts

`workspace protect` / `workspace release-protection` consume strict JSON and preview a canonical
digest. Clearing one protection does not override other blockers. `discovery adoption-draft`
requires project/environment/resource/revision/rationale and returns two proposed records; neither
is stored or accepted automatically.

## Optional metadata hooks

The user must explicitly configure a registered profile and workspace. Use `lifecycle hook`
with the matching `claude-hook`, `gemini-hook`, `codex-notification`, `acp-notification`, or
`copilot-session-event` format and `--accept-agent-report`. It consumes bounded stdin; no transcript
path is followed. A configured start hook may perform a best-effort bounded ancestry observation.

`telemetry hook` accepts Claude statusline, Codex rate-limit, Gemini headless session statistics
or Copilot context format for the corresponding registered integration. Output is empty in text
mode to avoid contaminating a statusline. It stores a normalized projection, not raw prompts.
There is no automatic edit of agent settings files or watched skill directories.

## Secrets

`secret history --project ... --environment ... --id ...` and `secret recovery-status` expose
metadata only. Rotation/revocation are typed effects. Secret input is hidden console input at
execution time, never JSON plans or command arguments. Local revoke is NOT remote token revoke.
