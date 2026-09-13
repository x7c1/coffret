---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "requireMetaLength" frontend/packages/domain/format/src/containerHeader.ts && ! grep -rq "U32_MAX - TAG_LENGTH" frontend/packages/domain/format/src/ && grep -rq "meta_section_too_long" frontend/packages/domain/format/src/container.test.ts && grep -q "spec: FM-11" backend/crates/domain/coffret-format/src/control/adversarial_length_tests.rs && ! grep -rq "command an allocation of nearly 4 GiB" backend/crates/domain/coffret-format/src/ && grep -q "bounds the absurd rather than the ordinary" backend/crates/domain/coffret-format/src/header.rs'
assignee: null
branch: task/0913-1836-let-both-decoders-refuse-the-same-container
created_at: 2026-09-13T18:36:47Z
updated_at: 2026-09-13T19:20:13Z
---

# fix(frontend): let both decoders refuse the same Container

## Overview

The register now states the ceiling on a Container's meta section: 64 MiB, and
it binds a writer as well as a reader (`FM-2`). Measured against that rule, the
two implementations of this format disagree — and they disagree in both
directions, which is worse than either alone.

| | reads | writes |
| --- | --- | --- |
| Rust | refuses past 64 MiB (`header.rs`, as the header is parsed) | refuses past 64 MiB (`layout.rs`, as the layout is drawn) |
| TypeScript | **no ceiling at all** | **refuses past 4.0 GiB** |

So a TypeScript writer can produce a Container that the Rust reader refuses —
sixty-four times over — and a TypeScript reader accepts one the Rust reader
refuses. A Container is the unit this product stores; two implementations that
disagree about which ones are valid is an interoperability defect, not a
hardening gap.

### 1. The TypeScript reader checks nothing

`frontend/packages/domain/format/src/containerHeader.ts`, in
`parseContainerHeader`:

```
    chunkSize: requireChunkSize(readU32BE(bytes, CHUNK_SIZE_OFFSET)),
    metaLength: readU32BE(bytes, META_LENGTH_OFFSET),
```

The line above it does exactly what the line below it does not. `requireChunkSize`
is a named validator with its own documentation explaining which rule it serves
(`FM-6`); `metaLength` is read raw. Give it the same treatment, and the same
shape — a `requireMetaLength` beside `requireChunkSize`, refusing with
`meta_section_too_long`, the kind the encoder already uses.

Refuse it where the header is parsed, before anything is allocated for the
claim, which is what `FM-2` says the ceiling is for and what the Rust reader
does.

### 2. The TypeScript writer uses the wrong ceiling

`frontend/packages/domain/format/src/encodeContainer.ts`:

```
  // The header records the padded section together with its tag in one 32-bit
  // field, so the ceiling a meta section fits under is that field's maximum
  // minus the tag.
  const metaLengthLimit = U32_MAX - TAG_LENGTH;
```

That reasoning is sound and answers a different question. It asks *what the
header field can record*; the rule asks *what a conforming writer may produce*.
The first is 4.0 GiB and the second is 64 MiB, so the encoder refuses only the
unrepresentable and not the non-conforming.

Use the register's number. The field-maximum argument stops being the ceiling;
whether it is worth keeping as a note about the field is a judgement — if it
stays, it must not read as the limit.

### 3. One number, named once

Both sides now need 64 MiB, and the interop suite needs it too. A second
spelling of a format constant is the defect this repository has fixed before
under other names. Put it where the format's other shared constants live and
have both paths read it.

### 4. Two doc defects the register's new rules leave behind

`FM-2` and `FM-11` were added to the register with the text they were lifted
from still in place beside the code:

- `backend/crates/domain/coffret-format/src/header.rs`, the doc on
  `MAX_META_LEN`: its first and last paragraphs are now near-verbatim with
  `FM-2`'s two new sub-bullets. Drop those two and leave a pointer; keep the two
  middle paragraphs, which are why the number is this one and which no rule
  carries.
- `backend/crates/domain/coffret-format/src/control/adversarial_length_tests.rs`:
  its module doc describes the per-kind ceilings, and its cases cite `FM-12`
  and `FM-15`/`FM-16`/`FM-17`, but nothing in the file cites `FM-11`, which is
  now the rule that states them. `docs/spec/README.md` asks that a `Form: test` rule be reachable by
  searching for its id; searching for `FM-11` does not reach the test that
  proves it.

## Out of scope

- **The control-object ceilings on the TypeScript side.** `FM-11`'s per-kind
  ceilings have the same shape as this defect, but the control path decodes
  through `decodeControlObject` rather than the Container header, and pulling it
  in doubles the change. The divergence is there: nothing on the TypeScript
  side holds a declared length against a kind's ceiling — `control/payload.ts`
  raises `control_payload_too_long` only for a padded length this runtime
  cannot address — so `FM-11`'s three ceilings go unenforced there, while the
  rest of `FM-11`, its framing and its padding condition, is kept.
- **Catch-up's unbounded retention, and the Drive transport applying a
  document's ceiling to a Storage Object.** Both are ceilings applied wrongly at
  a Storage boundary rather than in the format, and both come next.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The TypeScript reader refuses a header declaring a meta section past
      64 MiB, as `meta_section_too_long`, before it reads or allocates for the
      section; a test drives a header that declares more and asserts the
      refusal.
- [x] The TypeScript writer refuses to produce a meta section past 64 MiB, and
      no longer treats the header field's maximum as the ceiling.
- [x] The ceiling is one named constant that both the reader and the writer
      read, not a number written twice.
- [x] A test states the boundary from both sides — one at the ceiling that is
      accepted, one past it that is refused — the way
      `backend/crates/domain/coffret-format/src/decode/adversarial_length_tests.rs`
      does for the Rust reader.
- [x] `MAX_META_LEN`'s doc no longer restates `FM-2`'s new sub-bullets, and
      still carries why the number is this one.
- [x] `control/adversarial_length_tests.rs` cites `FM-11`.

### Manual / on-hardware (verified by a human before merge)

- [ ] Nothing here is observable in the running product: a Container with a
      64 MiB meta section is far past anything `freeze` produces. The interop
      suite is the check that matters, and it is automated.
