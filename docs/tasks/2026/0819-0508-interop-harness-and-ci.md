---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make interop && cd backend && cargo fmt --all -- --check && cargo build && cargo test && cargo clippy --all-targets -- -D warnings && cd ../frontend && pnpm install && pnpm -r build && pnpm -r typecheck && pnpm -r test && pnpm -r lint"
assignee: null
branch: task/0819-0508-interop-harness-and-ci
created_at: 2026-08-19T05:08:00Z
updated_at: 2026-08-19T08:40:00Z
---

# feat: verify Rust and TypeScript format interoperability by fixture exchange

## Overview

The repository now carries two independent implementations of the
storage format: `backend/crates/domain/coffret-format` (Rust, the
reference) and `frontend/packages/domain/format/` (`@coffret/format`,
TypeScript). This task adds the permanent proof that they interoperate —
each side decrypts what the other encrypted — as a fixture exchange run
in CI on every change. The exchange keeps the specification honest:
when it fails, either the spec or one implementation is wrong, and the
fix is a deliberate commit to whichever side is.

Both spec registers (`docs/spec/format/README.md`,
`docs/spec/key-derivation/README.md`) are normative; read them first.

Implementation shape:

- **New bin crate `backend/crates/apps/coffret-interop`** (the first
  `apps/` crate wired for this purpose; follow the workspace conventions
  and layer rules — `apps/` may do I/O, `domain/` crates stay
  byte-in/byte-out and are NOT modified by this task):
  - `coffret-interop generate --out <dir>` — writes a fixture set and a
    `manifest.json` describing it. The set must cover at least: a
    multi-Entry Container, a Container whose Entries are all empty (the
    empty-stream chunking choice both implementations pin by test), all
    three control-object kinds (Journal record, Keyring replica with a
    non-trivial replica position, Index Snapshot), a Key Envelope, and
    a stored Master Key form. The manifest carries everything a reader
    needs to open the fixtures and check them — hex-encoded Master Key,
    Container Keys and IDs, the Passphrase, object names, expected
    Entry contents and control-payload fields — but no derived
    expectations that would re-implement the format in the manifest.
  - `coffret-interop verify --in <dir>` — reads a fixture set of the
    same layout produced by the TypeScript side, opens every object
    with the manifest's key material, compares decoded contents against
    the manifest's expectations, and exits non-zero on the first
    mismatch with a message naming the fixture and field.
- **TypeScript interop runner** in `frontend/packages/domain/format/`:
  a vitest suite (e.g. `interop.test.ts`) that runs only when a fixture
  directory is supplied (environment variable or config), so plain
  `pnpm -r test` stays self-contained and green without fixtures. Given
  the Rust fixture directory it decodes every fixture and asserts the
  manifest's expectations; it then writes the reverse-direction fixture
  set (same layout, same manifest schema) for `coffret-interop verify`.
  Control-payload `body` comparisons are made on decoded CBOR fields,
  never raw bytes — the two encoders legitimately serialize maps
  differently.
- **Wiring**: a `make interop` target runs the three steps serially —
  `cargo run -p coffret-interop -- generate` into a scratch directory
  under `.tmp/`, the vitest interop suite against it, then
  `cargo run -p coffret-interop -- verify` on the TS output — and a new
  `interop` job in the CI workflow runs `make interop` on every push,
  alongside the existing `backend` and `frontend` jobs (unchanged).
  Fixture directories are scratch output and are never committed.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make interop` performs the full exchange — Rust `generate`, the
      TypeScript suite decoding those fixtures and emitting the reverse
      set, Rust `verify` accepting it — and exits zero (it is the first
      element of `check_command`).
- [x] The generated fixture set covers a multi-Entry Container, an
      all-empty-Entries Container, all three control-object kinds, a
      Key Envelope, and a stored Master Key form, each listed in
      `manifest.json` (the TypeScript suite asserts the manifest lists
      all covered kinds, so a dropped fixture fails the exchange).
- [x] The TypeScript interop suite is skipped without a fixture
      directory: `pnpm -r test` passes on a clean tree with no fixtures
      present (part of `check_command` after `make interop`'s scratch
      output, which lives under gitignored `.tmp/`).
- [x] `coffret-interop verify` fails loudly on a corrupted exchange: a
      unit or integration test flips one ciphertext byte in a fixture
      and asserts a non-zero exit / error naming the fixture.
- [x] Backend workspace gates stay green with the new crate:
      `cargo fmt --check`, `cargo build`, `cargo test`,
      `cargo clippy --all-targets -- -D warnings`.

### Manual / on-hardware (verified by a human before merge)

- [x] The `interop` CI job runs and passes on this PR.
- [x] `backend/crates/domain/*` and the public spec are untouched by
      this task's diff; any incompatibility found was recorded, not
      papered over.

## Out of scope

- Fixing spec gaps recorded by the previous task (Argon2id version,
  `derived_from` keys, first generation value, empty-stream chunking,
  object-name renderings) — those are deliberate follow-up spec edits.
- Storage adapters, Drive access, viewer integration.
- Per-kind control-object payload schemas beyond what the fixtures need
  as opaque caller-supplied fields.
