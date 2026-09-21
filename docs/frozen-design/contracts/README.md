# Reference contracts

Draft 2020-12 JSON Schemas. The `.invalid` IDs are stable local design identifiers; no online service is required or expected. Use local schema resolution.

These schemas validate **shape**, not authorization or truth. Domain code must additionally validate plan digests/expiry/identity, cross-project environment references, version compatibility, decision authority, graph cycles, temporal conflicts, source consent, unsafe URLs, path traversal/reparse changes, and content redaction. A field of type string can still contain a secret: schema validation does not replace privacy enforcement.

`fingerprint.evidence_all` and `protect_if` are identifiers for compiled allowlisted predicates, not arbitrary expressions. Unknown predicate or compiled operation IDs disable the rule. Rule updates cannot introduce arbitrary shell execution.

The action-plan schema intentionally allows no network access. Credentialed task execution uses a separate reviewed task-spec and consent path described in the master design; it is not a repair-rule escape hatch. The synthetic plan example is not executable, has fixture hashes, and does not represent a complete repair.

Decision `status` is a derived view for interchange. In the database, immutable revisions and transition events preserve authority history. Accepting an imported `status=accepted` field is forbidden without separate local authority validation.
