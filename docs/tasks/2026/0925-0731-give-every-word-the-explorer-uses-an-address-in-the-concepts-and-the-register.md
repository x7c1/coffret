---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/0925-0731-give-every-word-the-explorer-uses-an-address-in-the-concepts-and-the-register
created_at: 2026-09-25T07:31:34Z
updated_at: 2026-09-25T09:46:02Z
---

# docs: give every word the explorer and the device use an address in the concepts and the register

## Overview

A pass over the concept documents (`docs/concepts/`) and the spec
registers (`docs/spec/`): words the code and the wire already use that have
no definition, a concept that exists in types and commands but not on paper,
guarantees stated in a concept's Domain Rules that belong in a register, and
a few sentences that are slightly wrong. Every item names the
document that gains or loses a sentence; nothing here changes behaviour, and
the only code touched is comments and one TypeScript type name.

### 1. A concept document for mapping

A mapping has types (`Mapping`, `MappedRoots`), two CLI commands (`map`,
`mappings`), a wire field (`mapped` on `ListingDto` / `FolderDto`) and three
spec rules (EP-9, EP-12, CK-7), and no document: the meaning of a mapped root
lives only in a device-layer doc comment. Write `docs/concepts/mapping/README.md`
in the shape the other concepts use (Definition, Examples, Collocations,
Domain Rules, Related Concepts, Related Specification) — read three
neighbours (`entry-path`, `index`, `library`) before writing so the voice and
the section order match — and link it from `docs/concepts/README.md` and from
the Library and Entry Path concepts where they speak of "a mapped folder".
What the device layer's doc comment says about the root becomes a Domain Rule
here and the comment cites it.

### 2. Words on the wire with no home

- **`present` / `remote`** — `EntryState::Remote` and the wire's `"remote"`
  are the device's state words for an Entry, yet the concepts use `remote`
  only for Storage, and the Index concept's opposite of *present* is *absent*.
  Register `present` and `remote` as the Entry's on-device state, on the
  Entry or Index concept (choose by which one EP-10 and CK-7 already anchor
  to), and bind them to those two rules. Then rename the web hook whose type
  is a homonym: `useRemote.ts`'s `Remote<T>` becomes `Asked<T>` / `useAsked`
  (it means "a value the page asked for", nothing about Storage) — every
  reader in `frontend/packages/apps/web/src` follows.
- **`add`** — register it in the Library concept's Collocations, and add the
  Domain Rule that a file merely added is not *materialized* in EP-10's
  sense.
- **`supersede`** — the explorer's activity vocabulary uses it for a run that
  replaces another; the Container concept uses it for one Container replacing
  another. Give the explorer's sense an address that does not collide (a
  Collocation on the concept that owns runs, with a sentence saying which
  sense is which).
- **`explorer` / `reader` / `page` / `openable`** — the words are decided:
  *explorer* is the whole operating surface on a device (placement — EP-10,
  mapping, fill); *reader* is the state inside it that shows one Entry's
  sequence (decryption, page order, read-ahead); *page* and *openable* sit
  under reader; *viewer* is retired. Register them where the surface's
  concept lives (the Library concept's Collocations, or a short section the
  concept README points to), and rewrite the five live code comments that
  still say "viewer".
- **`renew`** — nothing says a grant expires or how a device gets a new one.
  Add a Domain Rule to the Storage concept and the Collocation
  *renew (a device's access to Storage)*, worded for a device and an account
  rather than for a Library, since where the grant lives is about to change.
- **`finding`** — the Library term (a non-error report about one file) is
  also used in code for "the judgement an error carries". Where the code
  means the latter, say *verdict* (comments and identifiers in the error
  modules only; count them first).
- **`worker` / `background task`** — the server says both for the same
  thing; pick the one the activity vocabulary already uses and apply it in
  comments and docs.
- **`claimed` / "the walk claimed"** — the canon verb is *represent* (EP-9)
  and `claim` is EP-11's word for something else; rewrite the comments in
  `local_scan/` that use it.

### 3. Guarantees that belong in a register

- **A catalog opened by two processes at once** is promised only in the
  Index concept's Domain Rules. Procedure belongs in a register: add one rule
  (under CK, or a new Index prefix if none fits — say which and why) and
  shrink the concept to a one-sentence citation.
- **The running state of fill / sync / freeze** (a device state that lives
  as long as the process) has no rule. LA is the register for what a running
  server holds; add the rule there.
- **A one-shot process and DK-3 / DK-4** — the CLI has no explicit lock and
  no idle lock; the process ending is the lock. Decide between adding to DK-1
  that a process-scoped unlock returns to locked when the process ends, or
  scoping DK-3 / DK-4 to long-running sessions; write the reason in the rule.
- **FM-9's `mime` paragraph** delegates its norm to "the server" — the only
  mention of the server in `docs/spec/`, with no rule id. Decide where the
  norm lives (a rule of its own, or none, leaving what an Entry opens as to
  the application that serves it) and make FM-9 say only what the format
  guarantees.
- **"replica index" / "position" / "health event"** — the register says
  *replica index* where code and concepts say *position*, and KL-15's *health
  event* has no home outside the register. Align the register's words with
  the concepts' and give *health event* a sentence in the Keyring concept.

### 4. Sentences that are slightly wrong

- Entry Path concept, Collocation `refuse`: its gloss (a name or path the
  Library will not take) is narrower than the register's use (EP-11 and
  EP-13 also refuse a placement). Widen the gloss.
- KD-11's sub-bullet ends "neither renames the other"; *renames* is not the
  word — find the one the concepts use for that relation.
- PK-3's sub-bullet says a table is "most of what they weigh", which is
  stronger than the code's claim; say what the code guarantees.
- The convention for citing a rule from code — `(spec: KD-4)` — is followed
  everywhere and written nowhere. Add two sentences to `docs/spec/README.md`
  saying the form and when a comment cites one.

### 5. Record only

The OC register still says *reclaim* where the concepts say *dispose*
(OC-7); the concepts already reconciled the term and the register sentence
reads correctly in context. Leave it, and say so in the report.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] `docs/concepts/mapping/README.md` exists, is linked from the concepts
      index and from the Library and Entry Path concepts, and the device-layer
      comment about a mapped root cites its Domain Rule
- [x] `present` / `remote`, `add`, `supersede`, `explorer` / `reader` /
      `page` / `openable`, and `renew` each have a Collocation or Domain Rule
      in a concept document, and no live comment says "viewer"
- [x] `Remote<T>` / `useRemote` are `Asked<T>` / `useAsked` in the web
      package
- [x] the catalog concurrency guarantee, the running state of fill / sync /
      freeze, and the one-shot process are each a numbered rule in a register
- [x] `docs/spec/README.md` states the `(spec: XX-n)` citation convention
- [x] the register uses *position* where the concepts do, and *health event*
      is defined in the Keyring concept

## Out of scope

- Where a grant lives (a device and an account rather than a Library): a
  separate change; `renew` is worded so that it survives it
- Any change to a variant's meaning, a route's answer, or the wire beyond the
  hook rename
