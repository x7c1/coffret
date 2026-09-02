---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -rqi 'adversar' backend/crates/domain/coffret-usecase && grep -rqi 'adversar' backend/crates/domain/coffret-format"
assignee: null
branch: task/0902-2350-bound-what-untrusted-lengths-may-allocate
created_at: 2026-09-02T14:55:00Z
updated_at: 2026-09-02T19:15:00Z
---

# fix(fetch): bound what untrusted lengths may allocate

## Overview

Storage is explicitly outside the trust boundary, and the AEAD does
catch every tampered byte — eventually. The problem is what happens
before "eventually". A Container header's `meta_len` is an
unauthenticated `u32`; `ContainerOutline`'s prefix length is `32 +
meta_len` with no ceiling; the partial-fetch path range-reads that many
bytes and `collect_exact` pre-allocates the full expected size. A
Storage response (or a provider account someone else tampered with)
that flips the first 32 bytes of an existing Container can therefore
make the explorer or CLI attempt an allocation of about 4 GiB before
any authentication runs. The whole-Container decode path grows a buffer
toward the same declared length, and the control-object path's
`into_bytes()` believes whatever total length the provider declares.
Authentication verifies bytes; it does not authenticate the resources
spent obtaining them.

Put explicit ceilings in front of every allocation an untrusted length
can command:

1. **Decide the legitimate maxima, in the format layer.** The meta
   section holds one entry table for one Container; the control
   objects (head, Journal record, Snapshot, Keyring) each have a
   realistic size envelope. Derive a ceiling for each from what the
   format can actually produce (with generous headroom and a note on
   future growth — a ceiling that merely restates today's typical size
   will be wrong tomorrow; one that bounds the absurd is right for
   years). The ceilings live beside the format types they protect, as
   named constants with the reasoning in their docs — they are format
   decisions, not transport tuning knobs.
2. **Refuse before allocating, and before the second request.** The
   outline/decode paths check the declared length against the ceiling
   as soon as the header is parsed — before `Vec::with_capacity`,
   before a range request is issued for the remainder. The refusal is
   the existing tampered/malformed vocabulary (a lying length IS
   tampering evidence), surfaced the way other undecodable Containers
   are today, per-entry where fetch already declines per-entry.
   Control-object reads bound `into_bytes()` the same way: a declared
   or actual size past the ceiling stops the read without buffering
   the excess.
3. **Do not trust the response to match its declaration either.** A
   response that overruns its declared length must stop consuming at
   the bound rather than growing; a response that underruns must fail
   cleanly (it already does — keep it pinned).
4. **Adversarial-store tests.** Against the in-memory store: a
   Container whose header declares `meta_len = u32::MAX` (fetch
   declines that Entry before any large allocation — assert on
   behavior, and keep the test cheap: it must not actually allocate
   gigabytes even on failure); a control object whose store-reported
   length exceeds the ceiling; a response that returns more bytes than
   declared; one that returns fewer. These live beside the existing
   conformance suites and run in `make check`.

Update the doc comments on the reading paths: "the AEAD catches
tampering" stays true, and now the sentence beside it says what is
checked *before* spending memory.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Every allocation commanded by an untrusted length (meta section,
      control objects, range reads) is preceded by a ceiling check;
      the ceilings are named constants in the format layer with
      reasoned docs.
- [x] A Container declaring `meta_len = u32::MAX` is declined before
      any allocation of that size, on both the partial-fetch and
      whole-Container paths, with the existing tampered/malformed
      refusal shape (the `adversar` grep gates pin the new tests).
- [x] Oversized control-object reads stop at the ceiling without
      buffering the excess; overrun and underrun responses both fail
      cleanly with tests.
- [x] An honest writer cannot reach the meta ceiling: segmentation
      keeps every Pack's entry table under it (a freeze of very many
      tiny files splits rather than fails), with a test.
- [x] An oversized control object among catch-up candidates is skipped
      the way an undecodable one is — it never leaves the Library
      permanently unopenable — with a test.
- [x] `make check` passes; the interop suite proves well-formed
      Containers and control objects are read byte-identically.

## Out of scope

- The explorer server's own resource envelope — how it bounds what a
  request may cost it — is separate work.
- Changing the storage format itself (no new header fields; the
  ceilings interpret the existing format).
- Rate limiting and retry behavior.
