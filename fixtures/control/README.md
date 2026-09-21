# Synthetic control-plane fixtures

Everything here is fabricated for tests. No real account, provider quota, billing record,
model score, credential, repository content or vendor session is included.

The two named vendor payload files mimic only fields verified in current primary docs.
They are not evidence that a live provider integration was exercised. The CLI imports are
explicit local files, never authenticated page scraping. `economics-vectors.json` supplies
expected cases consumed by authored Rust tests and separately checked in the reference lab.
Passing the Python oracle is **not** proof that the Rust implementation passed those vectors.
