# Implementation research register

SOURCES-V3.json maps public primary documentation to source paths. Documentation browsing
is not a provider/account call by Workstation. No authenticated account, native CLI, HTTP
client, secret provider or live session was exercised this pass. The user's Windows remains
untouched. Exact tested-version lists are deliberately empty.

Important corrections reflected in code: Grok's documented transport is `agent stdio` with
auto-update disabled, not a guessed `grok acp`; ACP tool locations use `path`; a Claude Stop
is a turn boundary, not SessionEnd; Anthropic costs use decimal cents, not USD; a Copilot
unlimited entitlement is not a finite allowance; a Docker GUI should not be killed with an
ephemeral query worker.

Existing dependency pins were inherited, with reqwest0.13.5 and serde_json arbitrary_precision
added from documentation. No real dependency graph or lockfile was resolved here. Source
approach remains realistic but unverified until the authorized build and interoperability
phase. This register does not establish that private consumer quota APIs exist.
