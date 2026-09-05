---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -q 'address space' docs/spec/format/README.md && ! grep -q 'caps far below' docs/spec/format/README.md && ! grep -q 'rejects an entry ending past the integer range the format admits' frontend/packages/domain/format/src/meta.test.ts && grep -q 'past the end of the address space' frontend/packages/domain/format/src/meta.test.ts"
assignee: null
branch: task/0905-1452-define-the-plaintext-streams-address-space-in-the-register
created_at: 2026-09-05T14:53:20Z
updated_at: 2026-09-05T15:23:13Z
---

# docs(format): define the plaintext stream's address space and ground FM-19's rationale

## Overview

The last format change (FM-19, which bounds every integer the format
carries below 2^63) rewrote FM-9's extent sub-bullet and, in doing so,
removed the only place the register used the phrase "address space" — it
had said "the plaintext stream's own 64-bit address space". The code still
speaks that language, and it is the right language: `Error::ExtentPastTheAddressSpace`
(`coffret-model`), `EntryExtent`'s docs ("ends inside the address space
the format admits"), `stream_extent.rs`, the `entry_extents` fixture modules
in `coffret-format` and `coffret-usecase`, a `meta/decode.rs` comment, four
older Rust test names (`an_extent_past_the_end_of_the_address_space_cannot_exist`,
`an_entry_extent_past_the_end_of_the_address_space_is_rejected` in three
crates, `a_row_whose_extent_passes_the_end_of_the_address_space_makes_the_catalog_unreadable`),
three assertion messages, and two TypeScript test titles. A reader who
follows the term from the code to the register now finds nothing. Rather
than rename twenty-odd sites, give the term its definition back where the
bound lives.

Two smaller register fixes ride along, both found while reviewing FM-19:

- FM-19's rationale says "an offset lies inside an object whose size a
  Storage caps far below". Nothing in the register or the concept documents
  says a Storage caps anything (the Storage concept promises nothing about
  capacity; PK-4's size target admits oversized singletons), so the
  sentence rests on a claim the documents do not make.
- The three TypeScript tests that reject an extent past the bound are
  titled two ways: `meta.test.ts` says "past the integer range the format
  admits" (renamed by the FM-19 change) while `control/indexSnapshot.test.ts`
  and `control/journalRecord.test.ts` still say "past the end of the address
  space", which is also what their Rust mirrors
  (`an_entry_extent_past_the_end_of_the_address_space_is_rejected`) say.

### 1. Define the term in FM-9 (`docs/spec/format/README.md`)

- In FM-9's extent sub-bullet (the one that now reads "its end —
  `offset + size` — is below 2^63, the bound FM-19 puts on every integer
  this format carries, so every entry has an end that is a position in the
  stream"), add one sentence that names the set: the positions below that
  bound are the plaintext stream's **address space**, and every extent lies
  inside it. Bold the term on its first use the way FM-18 bolds **app
  folder** and FM-12 bolds **role**. Keep the rest of the bullet as it is.
- FM-19's "positions as well as counts" sub-bullet may then say an entry's
  end is a position in the stream's address space, if that reads better than
  restating the bound — but do not restate FM-9's definition there.
- No new rule ID: this defines a term inside an existing rule, as FM-18
  defines **Library ID** inside FM-18.

### 2. Ground FM-19's size rationale (`docs/spec/format/README.md`)

- Replace "an offset lies inside an object whose size a Storage caps far
  below" with a statement the documents back. An offset counts bytes inside
  one Storage Object, and 2^63 of them is eight exbibytes — arithmetic, not
  a claim about any provider. Keep the sentence's job (nothing the format
  counts approaches the bound) and its three examples (generation, epoch,
  offset).

### 3. Align the TypeScript test titles (`frontend/packages/domain/format/src`)

- Retitle `meta.test.ts`'s "rejects an entry ending past the integer range
  the format admits" to the wording its two siblings and its Rust mirror
  use: "past the end of the address space" (the exact phrase the check
  command greps). The comment above it already says what the bound is.
- Leave every other TypeScript test titled "past the integer range the
  format admits" alone: those are about generations, epochs, and payload
  integers, not extents, and the phrase is the right one for them.

### 4. Leave the code's vocabulary as it is

With the term defined in the register, `Error::ExtentPastTheAddressSpace`,
the Rust test names, the docs that say "inside the address space", and the
docs that say "the positions the format admits" all mean one defined thing.
Do not rename or reword them; a meaning-preserving sweep is not this task.
The one exception is a doc comment that still describes the address space
as 64-bit (grep `64-bit` under `backend/crates` and
`frontend/packages/domain/format/src` for anything tied to an extent) — the
FM-19 change removed those, so expect none, but fix one if it turns up.

### Out of scope

- Renaming `Error::ExtentPastTheAddressSpace` or any test.
- Concept documents (`docs/concepts/`): the term is format mechanics owned
  by the register, not domain vocabulary.
- Any other rule; FM-19's `Form: test` migration.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] FM-9 defines the plaintext stream's **address space** as the positions
      below FM-19's bound, and the phrase "address space" is back in the
      register
- [x] FM-19's rationale no longer claims a Storage caps object sizes
- [x] The three TypeScript extent-rejection tests share the title wording
      "past the end of the address space", matching their Rust mirrors
- [x] `make check` (backend fmt / build / test / clippy, frontend, interop) is
      green
