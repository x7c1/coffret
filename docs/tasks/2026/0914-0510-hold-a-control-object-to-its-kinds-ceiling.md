---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, concept-alignment, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "control_object_too_long" frontend/packages/domain/format/src/'
assignee: null
branch: task/0914-0510-hold-a-control-object-to-its-kinds-ceiling-in-both-implementations
created_at: 2026-09-14T05:10:00Z
updated_at: 2026-09-14T06:21:24Z
---

# fix(frontend): hold a control object to its kind's ceiling in both implementations

## Overview

`FM-11` states a ceiling per kind of control object and says who holds to it:

> How long a control object may be is bounded by its kind: a Journal record
> 256 MiB, an Index Snapshot 512 MiB — ordinary or activation — and a Keyring
> 64 MiB. … So the declared length is held against its kind's ceiling first, and
> a reader that has only the name holds it against the largest ceiling that name
> admits (FM-12), the kind being inside the answer it is deciding whether to
> take. **A writer holds itself to the same ceilings, so an object a conforming
> writer produces is one a conforming reader takes.**

The Rust implementation does that from both ends: `control/ceiling.rs`'s
`check_control_object_len` is called by `control/decode.rs` and by
`control/encode.rs`.

**The TypeScript implementation does neither.** `control/decode.ts` checks the
name against the header carefully — `FM-12`'s admission table, the generation,
the replica position, all before the key is used — and never looks at a length.
`control/encode.ts` refuses a name/kind pairing the same table does not list,
and never looks at a length either.

So the two implementations disagree about what a valid control object is, across
three kinds and up to 512 MiB.

### This is the same defect that was fixed one layer down

A Container's meta section had exactly this shape: `FM-2` gave it a ceiling, the
Rust side held both ends to it, and the TypeScript side did not. That was fixed
by giving the TypeScript package a `meta_section_too_long` code raised from both
ends, and `errors.ts` still carries the comment that says why:

> Both ends raise it: the encoder refuses to lay out a Container whose entry
> table would need more, and the decoder refuses a header that declares more,
> before anything is sized by the declaration (FM-2).

Do the same for control objects, one layer up. The new code is
`control_object_too_long`.

Do not reuse `control_payload_too_long`. That code is about something else:
`payload.ts` raises it when a padded length passes `Number.MAX_SAFE_INTEGER`,
which is about what this platform can address and not about any ceiling a build
chose.

## What this change has to decide

**The reader side is not the same question in the two languages, and the task
does not settle it for you.**

In Rust, the ceiling is consulted *before the bytes are taken in* — that is what
`FM-11`'s "a reader that has only the name holds it against the largest ceiling
that name admits" is for, and `ceiling.rs` has a `max_control_object_len_at` for
exactly that. In TypeScript, `decodeControlObject` receives a `Uint8Array` that
somebody has already read, so refusing at that point does not save the memory
`FM-11` is protecting.

That does not make the check pointless — it is what keeps the two
implementations agreeing about what a valid object is, which is the whole reason
a second implementation exists. But it does mean you have to decide what to do
about the name-only ceiling:

- export it so a caller that *does* fetch can hold to it before reading, or
- leave it out and say in the code why there is no call site for it in this
  package today.

Pick one and say which in the code. **Do not invent a fetch path in the format
package to give it a caller.**

## Out of scope

- **The Rust side.** It already does this, and `control/adversarial_length_tests.rs`
  already holds it to it.
- **Widening or narrowing any ceiling.** The three numbers are `FM-11`'s and
  changing one is a change to its payload schema's own rule, which `FM-11` says
  in as many words.
- **`control_payload_too_long`.** It stays what it is.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The TypeScript encoder refuses to lay out a control object longer than its
      kind's ceiling, and the decoder refuses one it is handed, both raising
      `control_object_too_long`.
- [x] The three ceilings in the TypeScript package are the three `FM-11` states,
      and a test would fail if one of them drifted from the Rust constant.
- [x] An object at exactly its kind's ceiling is accepted by both ends — the
      refusal is for *past* the ceiling, as it is in Rust.
- [x] Whatever is decided about the name-only ceiling is decided in the code,
      with the reason written where it is.
- [x] `control_payload_too_long` still means what it meant, and nothing that
      raised it now raises the new code instead.
- [x] `errors.ts` says of the new code what it says of `meta_section_too_long`:
      which ends raise it and what each is protecting.

### Manual / on-hardware (verified by a human before merge)

- [ ] Nothing here is observable on hardware. The ceilings are exercised by the
      package's own tests; no Library large enough to reach one is needed, and
      none should be made.
