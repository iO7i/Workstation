# Non-executable templates for the implementation-first release

All example hashes, identities, accounts and paths are synthetic. These are **not approvals**
and were not applied. After the user authorizes Windows certification, use actual executable
hashes/version observations and review exact project/environment scope. Never place secret
values in these JSON files. Profile authentication maps environment variable names to approved
Atlas resource IDs, whose local DPAPI contents remain outside these documents.

`adapter` follows Rust serde snake_case (`one_password`, `cursor_acp`, `grok_acp`).
BillingProvider `OpenAiCosts` serializes as `open_ai_costs`.

Preparation creates an immutable expiring plan, not an external operation. Application requires
both the exact printed plan digest and `--acknowledge-uncertified-execution`. Do not execute it
until the user's separate authorization. These flags do not certify the operation or authenticate
a hostile process running as the same Windows user.

## ACP authentication profiles

For ACP adapters, do not guess an authentication method from documentation alone. First run a
read-only protocol-health query against the exact pinned executable and review its
`auth_method_ids`. If a noninteractive method is required, register that exact advertised ID in
`auth_method`. Do not auto-login, scrape browser cookies, or silently choose another method.

Native audit example: installed Grok 0.2.112 advertised `grok.com`, while public examples for
other environments show identifiers such as `cached_token` or `xai.api_key`. The installed
protocol response is authoritative for that registered binary.
