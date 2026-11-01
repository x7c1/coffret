---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design, spec-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1102-0423-bound-the-argon2id-cost-a-stored-master-key-may-ask-for
created_at: 2026-11-01T19:23:00Z
updated_at: 2026-11-01T21:12:28Z
---

# fix: bound the Argon2id cost a stored Master Key may ask for

## Overview

The stored Master Key form (KD-9, magic `CFMK1`) records its own Argon2id
memory, iterations and parallelism. A reader takes whatever values Argon2id
itself accepts (`backend/crates/domain/coffret-format/src/stored_master_key/layout.rs`,
`argon2_params.rs`), runs the derivation, and only then checks the AEAD
(`unlock.rs`). Because the parameters are bound as associated data (KD-7), a
lowered cost is caught — but a raised one is only caught after the work is
done. A stored form edited to memory 4,194,304 KiB and iterations
`u32::MAX` is accepted by `StoredMasterKey::from_bytes` and would exhaust
memory or never finish before the Passphrase is ever judged.

Separately, `backend/crates/apps/coffret-device/src/stored_master_key_file.rs`
reads the whole file with `fs::read` before the format checks its length.

## Decided bounds

The reader accepts a stored form only within these ceilings, inclusive:

| Parameter | Writer's value (`Argon2Params::INITIAL`) | Reader's ceiling |
|---|---|---|
| memory | 19,456 KiB | 262,144 KiB (256 MiB) |
| iterations | 2 | 8 |
| parallelism | 1 | 4 |

No combined ceiling: the two above already bound the work to 256 MiB × 8
passes. These are ceilings on what is accepted, not what is used — unlock
still derives with the recorded values. Every stored form coffret has
written uses `INITIAL`, so no existing key is affected.

## Changes

- **Spec.** In `docs/spec/key-derivation/README.md`, add to KD-9's reader
  rules that a reader rejects, before any derivation, a stored form whose
  memory, iterations or parallelism exceed the ceilings above, and state the
  values there. Note in KD-6 that later strengthening stays within these
  ceilings (raising them is a spec change). Regenerate whatever the repo
  derives from the spec, if anything.
- **Rust.** Enforce the ceilings where the stored form is parsed
  (`StoredMasterKey::from_bytes` / its layout), so no caller can reach
  `derive` with an out-of-bound cost. A refusal is its own error variant that
  names the parameter and the ceiling, distinct from `AuthenticationFailed`,
  and it never retries with lower values. Keep `Argon2Params` able to build
  the writer's values; the writer must never produce a form the reader
  rejects (assert `INITIAL` is within the ceilings).
- **File read.** `stored_master_key_file.rs` reads at most the format's
  largest valid length + 1 byte (take the read handle's `take`), and rejects
  anything longer without reading the rest.
- **TypeScript.** `frontend/packages/domain/format/src/storedMasterKey/`
  implements the same form; apply the same ceilings and refusal there, and
  keep the Rust/TS interop tests in agreement (`interop.test.ts`).
- **CLI / desktop messages.** Where the refusal reaches a person (unlock in
  the CLI or the desktop app), say that the stored key asks for more work
  than coffret allows and that the Passphrase was not checked. No path should
  read it as a wrong Passphrase.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A stored form at each ceiling still opens (round trip with cheap salt;
      do not run a 256 MiB derivation in unit tests if it is slow — test the
      check, and keep one round trip at `INITIAL`).
- [x] One above each ceiling (memory 262,145, iterations 9, parallelism 5)
      and the values of a form edited to exhaust a reader (4,194,304 KiB, `u32::MAX`) are refused before
      `derive` is called, in Rust and in TypeScript. Prove "before derive"
      without a large allocation (e.g. the refusal returns from parsing).
- [x] An oversized stored-form file is refused after reading at most the
      bound, in a test.
- [x] Spec KD-6 / KD-9 state the rule and the values; tests cite them.
- [x] `make check` passes.
