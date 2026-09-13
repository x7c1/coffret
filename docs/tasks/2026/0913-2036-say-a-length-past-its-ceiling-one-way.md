---
status: completed
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "a_length_past_the_ceiling_is_refused_the_same_way_everywhere" backend/crates/ && grep -rq "an_upload_that_declares_no_length" backend/crates/apps/coffret-server/tests/ && grep -rq "says_which_deadline_it_gave_up_at" backend/crates/apps/coffret-server/src/refresh/ && grep -rq "a_meta_section_a_writer_would_lay_out_past_the_ceiling_is_refused" backend/crates/domain/coffret-format/src/'
assignee: null
branch: task/0913-2036-say-a-length-past-its-ceiling-one-way
created_at: 2026-09-13T20:36:19Z
updated_at: 2026-09-13T21:27:28Z
---

# refactor(backend): say a length past its ceiling one way

## Overview

The register now states this product's ceilings and the code enforces them.
What is left is that the code does not say the same thing the same way, and that
three of the refusals nothing checks.

### 1. One judgement, two vocabularies

Two variants express the identical judgement — *a declared length exceeded a
bound* — in different words and with differently-named fields:

```
coffret-usecase::Error::ObjectTooLarge  { declared: u64, ceiling: u64 }
coffret-format::Error::MetaSectionTooLong { declared: u64, limit:   u64 }
```

`Large` and `Long`, `ceiling` and `limit`. A reader crosses this boundary
constantly — `coffret-usecase` calls into `coffret-format` on every decode — and
has to notice that two spellings mean one thing.

Pick one word for each and use it. `ControlObjectTooLong` is the same
judgement and should agree. Two others share the suffix and are **not** the same
judgement, so check each before drawing it in: `StreamTooLong` is raised for a
sum this code added up running past what the format admits (FM-19's 2^63) or
simply overflowing `u64`, and `ControlPayloadTooLong` for a padded length this
platform cannot address. Neither names a bound a build chose. If one keeps its
name for that reason, say so where it is declared.

A test should hold the vocabulary together rather than leaving it to review.
Name it `a_length_past_the_ceiling_is_refused_the_same_way_everywhere`.

### 2. A doctest that teaches the wrong scale

`backend/crates/gateway/s3-store/src/lib.rs`, in the module's example:

```
//! // Containers carry opaque names; the recognizable ones are control objects'.
//! let name = "0123456789abcdef0123456789abcdef.cfrt";
//! let object = store.put(name, ByteStream::from(b"ciphertext".to_vec())).await?;
//! // A read says how much it is willing to take in: Storage is outside the
//! // trust boundary, so the size of an answer is a claim until something
//! // inside it authenticates.
//! let bytes = store.get(&object, None).await?.into_bytes_within(4096).await?;
```

The example is internally consistent — it stores eleven bytes and reads within
four kilobytes — but it is framed as reading a **Container**, and a Container is
a Pack of photographs. Someone who copies the shape writes a reader that refuses
every real one.

The comment above the read is right about *why* the ceiling exists. What is
wrong is the pairing: a Container's name, and a ceiling three orders of
magnitude below any Container. Fix the example so its ceiling and its subject
belong together. Either is available — make the example read what 4 KiB suits,
or give the Container read a ceiling a Container fits under — but say which the
number is, because the next reader will copy it.

**This one has no gate.** It is a judgement about whether an example teaches the
right thing, and a `grep` for a number would forbid a correct fix that keeps it.
It is no less required for having no gate.

### 3. Three refusals nothing checks

- **`LA-11`'s no-length case.** The register now states that the room question
  asks for what the request declared it was bringing, "and one part's ceiling
  where it declared nothing". So a chunked upload of one kilobyte is refused on
  a volume with less than a gigabyte free. That is a real thing a person meets,
  stated in the register, and no test drives it. Name it
  `an_upload_that_declares_no_length`.
- **`gave_up` at startup.** `backend/crates/apps/coffret-server/src/refresh/catch_up_at_startup.rs`
  emits an `error!` carrying `deadline_ms`, and its doc reasons carefully about
  what a reader of that log should conclude — "whether to look at their network
  or at this constant". **The module has no tests at all.** `retry/tests.rs` and
  the Drive gateway's `logging_tests.rs` both show the shape. Name it
  `says_which_deadline_it_gave_up_at`.
- **The writer side of the meta-section ceiling.** `layout.rs` refuses a
  Container whose entry table would need a longer meta section, and no test in
  the Rust tree raises it — `MetaSectionTooLong` is exercised only from the
  reader. The TypeScript side pins both, so the coverage is uneven in the
  direction nobody expects. Name it
  `a_meta_section_a_writer_would_lay_out_past_the_ceiling_is_refused`.

## Out of scope

- **The TypeScript control path has no per-kind ceilings**, so `FM-11` is
  unenforced there while Rust checks it on both encode and decode. It is the
  same defect as the Container header's, one layer over, and it is a behaviour
  change through a different entry point rather than a matter of vocabulary.
- **Nothing runs `cargo doc`** — not `make check`, not CI — so rustdoc warnings
  are unguarded. That belongs with the other guard this repository is adding to
  CI, not here.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] One word names a length past a ceiling, and one word names the bound it
      passed, across `coffret-usecase` and `coffret-format`. A test named
      `a_length_past_the_ceiling_is_refused_the_same_way_everywhere` holds them
      together.
- [x] `StreamTooLong` is judged on its own terms, and whatever is decided about
      it is stated where it is declared.
- [x] The `s3-store` module example no longer pairs a Container's name with a
      ceiling no Container fits under, and says which its number is.
- [x] A test named `an_upload_that_declares_no_length` drives an upload with no
      `Content-Length` and asserts the room question is asked for one part's
      ceiling, as `LA-11` states.
- [x] A test named `says_which_deadline_it_gave_up_at` asserts the startup
      catch-up's abandonment carries `deadline_ms`.
- [x] A test named
      `a_meta_section_a_writer_would_lay_out_past_the_ceiling_is_refused`
      exercises the writer side of `MAX_META_LEN`.
- [x] No refusal changes: every rename keeps the same condition raising the same
      judgement, and no test that passed before is deleted rather than renamed.

### Manual / on-hardware (verified by a human before merge)

- [ ] Nothing here is observable at runtime beyond the words in two refusal
      messages. No manual check is needed.
