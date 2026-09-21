# Version 5 examples

These files demonstrate public, sanitized durable-runtime inputs. Replace placeholder IDs with values returned by your own Workstation home. Do not paste credentials, prompt bodies, private paths, or provider transcripts into an input file.

`verification-contract.json` allows one source file to change, forbids the test file from changing, and runs one previously registered task. Preparing the contract returns an approval digest; executing it is a separate command and requires explicit task-execution acknowledgement.

```powershell
$W = '.\target\release\workstation.exe'
& $W --home D:\WorkstationHome --json effect runtime baseline --id RUN_ID --acknowledge-content-hashing
& $W --home D:\WorkstationHome --json effect runtime verify --id RUN_ID --contract .\examples\v5\verification-contract.json
& $W --home D:\WorkstationHome --json effect runtime status --id RUN_ID
```

Review `docs/DURABLE-RUNTIME-IMPLEMENTATION.md` before preparing or applying a real continuation effect.
