# Economics data paths

The original domain layer remains: context runway, subscription runway and dollars are
separate meters. Forecast horizons, reset/correction/sparse/gap cases, plan fit, counterfactuals,
model filters and Pareto comparisons remain advisory, with provenance and unknown coverage.

New concrete source paths:

1. An explicitly approved Codex App Server `account/rateLimits/read` request is normalized
   through the documented rate-limit shape, persisted as **observed local response**, not
   inferred provider identity. Threads are not automatically considered active.
2. Fixed OpenAI organization costs and Anthropic organization cost-report GET requests use
   a scoped admin-key reference, bounded pages/body/time, no proxies/redirects/cookies and
   no automatic rate-limit retry. Cached output stays separate from subscription allowances.
3. Copilot SDK quota output has an explicit import normalizer. It is not a live SDK client.
   Unlimited entitlement is not fabricated as a finite allowance.
4. Strict numeric CSV provides an explicit fallback where a real interface is unavailable.
   Imported observations remain imported/partial. No authenticated consumer-page scraping.

OpenAI monetary values are decimal USD; Anthropic cost-report values are decimal cents.
Arithmetic parses the original decimal/scientific representation into checked integer
nano-USD, handles the unit conversion, and rejects overflow or finer unsupported precision
instead of silently rounding. Currency must be USD. Duplicate/overlapping/negative buckets
are rejected. This does not create permission to redistribute external benchmark datasets.

Costs are scoped to the organization selected by the credential, **not automatically
attributed to the Workstation project**. Provider exclusions/delay remain disclosed. They
are neither a settled invoice nor a numeric conversion of a consumer subscription quota.
A cached or explicitly imported value does not become provider-verified because it is in SQLite.

No live quota stream, invoice, provider account, SDK or benchmark feed was exercised here.
Consumer exact-quota APIs for several vendors remain absent/unimplemented; no fake combined
intelligence budget is displayed.
