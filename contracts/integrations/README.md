# Implementation v3 input authoring contracts

These four schemas describe integration profiles, approved tasks, typed effect requests
and descriptive project manifests. The Rust validators additionally enforce local paths,
provider-specific auth names, shell/script restrictions, project/environment relationships,
state identity, expiry and bounded values. A JSON-schema pass does not grant approval.

The supplied templates have synthetic paths/digests and must not be executed unchanged.
No schema validation suite, Rust compiler or native application was run for this delivery.
These files are authored source artifacts, not a generated proof of parity with Serde.

Normal metadata records retain their existing raw-file approval hash. New registrations
and manifests use a canonical typed JSON digest returned by their own preview. Effects use
the digest of the stored immutable plan. Never reuse one approval digest across those paths.
