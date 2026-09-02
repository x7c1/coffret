---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && cd backend && ! (cargo tree | grep -E 'h2 v0\\.3|rustls v0\\.21|rustls-webpki v0\\.101')"
assignee: null
branch: task/0902-1153-drop-the-legacy-tls-stack-and-its-advisories
created_at: 2026-09-02T02:53:32Z
updated_at: 2026-09-02T03:41:00Z
---

# fix(deps): drop the legacy TLS stack and its advisories

## Overview

A vulnerability cross-check of `Cargo.lock` against the OSV database turned
up one cluster that matters for a product whose security story leans on TLS
to Storage, plus two smaller items:

- **`rustls-webpki 0.101.7`** carries three certificate-validation
  advisories — URI name constraints incorrectly accepted
  (RUSTSEC-2026-0098), name constraints accepted for wildcard names
  (RUSTSEC-2026-0099), and a reachable panic in CRL parsing
  (RUSTSEC-2026-0104) — all fixed only in the 0.103 line (the rustls 0.23
  companion). It reaches the build through `rustls 0.21`, which
  `aws-smithy-http-client 1.3.0` pulls as its **legacy hyper 0.14 / rustls
  0.21 connector**. The same legacy stack pulls **`h2 0.3.27`**, whose
  unbounded-empty-DATA-frames DoS (RUSTSEC-2026-0258) is fixed only in the
  0.4 line.
- **`lru 0.16.4`** (a potential use-after-free from missing panic safety in
  `LruCache::pop()`, RUSTSEC-2026-0253, fixed in 0.18.2) arrives via
  `aws-sdk-s3 1.142.0`.
- **`paste 1.0.15`** is only an unmaintained-crate notice, and only reaches
  the tree through `image`'s AVIF/EXR encoders inside `coffret-fixtures` —
  a test-fixture generator that encodes simple images.

The workspace already builds `aws-lc-sys`, so the modern rustls 0.23 /
aws-lc stack is partly present; the legacy connector is riding along
through default features. Concretely:

1. **Retire the legacy AWS TLS stack.** Reconfigure the `aws-config` /
   `aws-sdk-s3` / smithy dependency features (in the workspace
   `backend/Cargo.toml` and the crates that consume them — `coffret-device`,
   `gateway/s3-store`) so the SDK uses the hyper 1 + rustls 0.23 (aws-lc)
   HTTP client and the hyper 0.14 / rustls 0.21 / rustls-webpki 0.101 / h2
   0.3 lineage drops out of `cargo tree` entirely. Prefer the smithy
   project's supported feature path over hand-rolling a connector. Verify
   the Drive gateway's `reqwest` transport is not on the legacy lineage
   either (fix its features the same way if it is).
2. **Chase `lru`.** Update `aws-sdk-s3` (and its smithy family in lockstep)
   to a release that depends on a fixed `lru`; if the newest SDK still pins
   the vulnerable line, record that plainly in the Cargo.toml comment and
   leave the advisory to upstream — do not vendor or patch.
3. **Trim the fixtures image stack.** Reduce `image`'s features in
   `coffret-fixtures` to the codecs the generator actually uses; if that
   drops the AVIF/EXR encoders (`rav1e` / `pulp` and with them `paste`),
   the unmaintained-crate notice disappears as a side effect. If the
   generator genuinely needs them, leave them and say so.
4. **Prove the runtime still works.** `make check` covers unit and interop;
   the S3 conformance suite against MinIO and the Drive store integration
   tests are the real proof that swapping the HTTP client changed nothing
   observable — run what is runnable locally (`make s3-store-it` uses
   Docker MinIO) and rely on the CI `s3-store` job for the gate. Watch for
   behavior-version APIs the SDK update may deprecate.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `cargo tree` no longer contains `h2 v0.3`, `rustls v0.21`, or
      `rustls-webpki v0.101` (the negated grep gate in `check_command`
      matches all three today).
- [x] `make check` passes with the reconfigured features — the S3 and Drive
      gateways compile and their unit/conformance tests run against the
      hyper 1 / rustls 0.23 client.
- [x] The `lru` and `paste` outcomes are explicit: either the advisory-fixed
      versions appear in `Cargo.lock`, or a one-line comment beside the
      dependency records why they cannot yet (upstream pin), with nothing
      silently left as-is.

## Out of scope

- Any change to the gateways' request/retry semantics or the `ObjectStore`
  port — this is a dependency/feature reconfiguration only.
- Adding a recurring dependency-audit job to CI (its own decision).
- Hardening work unrelated to dependency versions — this task changes
  dependency features and versions only.
