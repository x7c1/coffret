---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -q 'KD-11' docs/spec/key-derivation/README.md && grep -q 'recovery_codes' backend/crates/apps/coffret-interop/src/manifest/mod.rs"
assignee: null
branch: task/0830-0510-encode-the-recovery-code
created_at: 2026-08-30T05:10:00Z
updated_at: 2026-08-30T06:50:00Z
---

# feat(backend): encode the Master Key and its epoch as a Recovery Code

## Overview

The concept document `docs/concepts/recovery-code/README.md` defines the
Recovery Code as the human-transcribable encoding of the Master Key and its
epoch — the canonical backup of the key and the way it is carried to a new
device — but no encoding exists anywhere: there is no `RecoveryCode` type in
Rust, nothing in `@coffret/format`, and no rule in the spec register
(`docs/spec/key-derivation/README.md` ends at KD-10, the sealed token cache).
Two earlier tasks deferred it explicitly
(`docs/tasks/2026/0818-1554-key-derivation-and-control-framing.md:138`,
`docs/tasks/2026/0819-0319-typescript-format-implementation.md:155`). The
upcoming `coffret init` command must print one at the end of Library
creation, and a `join` command must accept one, so the encoding has to be
settled first, as a format-level rule with two implementations like every
other byte form.

Settle it as **Bech32m** (BIP-350) — the same family the Argon2id stored form
and the token cache sit beside in the KD register:

- Human-readable part `coffret`, separator `1`, then the data part.
- Payload before the 5-bit regrouping: `version (1 byte) = 0x01` ‖
  `epoch (8 bytes, big-endian)` ‖ `Master Key (32 bytes)` — 41 bytes, the
  same `Master Key ‖ epoch` pair KD-9's stored form protects, ordered with
  the epoch first so a future version byte can change what follows. 41
  bytes regroup into 66 five-bit characters with 2 zero padding bits, and
  the 6-character checksum brings the whole string to 80 characters
  (`coffret1` + 72), within Bech32's 90-character limit.
- A writer emits lowercase. A reader strips ASCII whitespace and hyphens
  (the user will have written the code down in groups), accepts a string
  that is entirely lowercase or entirely uppercase and rejects mixed case,
  verifies the Bech32m checksum, and rejects a wrong human-readable part, a
  payload that is not exactly 41 bytes, non-zero padding bits, or an
  unknown version. Every rejection is a typed error that says which check
  failed; a code that fails any check yields no key material.
- Display form for printing: the data part in groups of four characters
  separated by single spaces (`coffret1 qpzr y9x8 …`). The grouping is
  presentation only — the parser above accepts it because it strips
  whitespace — so it lives with the encoder as a formatting helper, not in
  the byte form.

Why Bech32m over a word list: 80 characters on one line versus ~30 words;
the alphabet already excludes the confusable `1`, `b`, `i`, `o`; the BCH
checksum detects up to four substitutions and, unlike a word-list checksum,
can point at the offending position; and it needs no bundled word list in
either implementation. The Rust side uses the `bech32` crate (add it to
`[workspace.dependencies]` in `backend/Cargo.toml`, exact version pinned
like the others); the TypeScript side implements the ~80 lines of Bech32m in
place under `frontend/packages/domain/format/src/` rather than adding a
runtime dependency — `@coffret/format` currently depends only on
`@noble/*`, `cborg` and `hash-wasm`, and a second implementation written
from the rule is the point of the exchange.

Concretely:

1. **Spec.** Add **KD-11** to `docs/spec/key-derivation/README.md` stating
   the encoding above in the same style as KD-9 and KD-10 (a byte table for
   the payload, then the Bech32m parameters and the reader's rejections),
   *(Form: test)*. Extend the KD row of the Mechanisms table in
   `docs/spec/README.md` so it names the Recovery Code encoding.
2. **Rust, `backend/crates/domain/coffret-format`.** A `RecoveryCode` type
   (its own module, one type per module as the crate does elsewhere) with
   `encode(&MasterKey, MasterKeyEpoch) -> RecoveryCode`,
   `parse(&str) -> Result<RecoveryCode>`, accessors for the Master Key and
   epoch, a `Display` that prints the bare lowercase string, and the grouped
   printing helper. Rejections are new variants of the crate's `Error`
   following the conventions the existing stored-form and token-cache
   variants use (typed causes, no stringified errors, no `PartialEq` on
   errors). The Master Key inside the parsed value must not be `Debug`
   -printed or otherwise leak — reuse whatever `MasterKey` already does for
   that.
3. **TypeScript, `@coffret/format`.** `encodeRecoveryCode` /
   `decodeRecoveryCode` beside `storedMasterKey/`, with the same rejections
   as typed errors in `errors.ts`, and unit tests mirroring the Rust ones.
4. **Interop.** Extend the fixture exchange in
   `backend/crates/apps/coffret-interop` (a `recovery_codes` list in the
   manifest, `generate/` writing codes for the fixture Master Keys and
   epochs, `verify/` parsing what the TypeScript side writes back) and the
   TypeScript side of `interop.test.ts` / `interop.testing.ts`, so `make
   check` (which runs the `interop` target) fails on any disagreement.
5. **Concept doc.** Update `docs/concepts/recovery-code/README.md` so the
   Definition cites KD-11 for the form and the Domain Rules say what a
   reader rejects (a leaked or mistyped code cannot yield a wrong key
   silently). Keep it to meaning and guarantees; the byte form stays in the
   spec.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `docs/spec/key-derivation/README.md` contains a KD-11 rule for the
      Recovery Code (the `grep -q 'KD-11'` gate is appended to
      `check_command`; it matches nothing today).
- [x] `coffret-format` unit tests cover: encode → parse round-trips the key
      and epoch for epoch 1 and for a large epoch (`u64::MAX`); the encoded
      string is exactly 80 characters, lowercase, and starts with
      `coffret1`; the grouped form parses back; an uppercase copy parses;
      mixed case, a flipped character (checksum), a wrong human-readable
      part, a 40- or 42-byte payload, non-zero padding bits, and version
      `0x02` are each rejected with the variant naming that check, and none
      of them yields a key.
- [x] `@coffret/format` unit tests cover the same matrix, and the interop
      exchange carries at least two Recovery Code fixtures in each
      direction (the `grep -q 'recovery_codes'` gate on the interop
      manifest is appended to `check_command`; it matches nothing today).
- [x] `docs/concepts/recovery-code/README.md` cites `KD-11`.

## Out of scope

- Reading a Recovery Code into a device (`coffret join`) and printing one at
  Library creation (`coffret init`) — the CLI tasks that follow.
- Master Key rotation minting a new code (MR-4) — the encoding is
  epoch-carrying already; the rotation flow is unimplemented and stays so.
- A QR or other machine-readable rendering.
