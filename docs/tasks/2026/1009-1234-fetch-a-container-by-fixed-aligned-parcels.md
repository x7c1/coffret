---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -q 'PK-19' docs/spec/pack-construction/README.md && grep -q 'PK-20' docs/spec/pack-construction/README.md && grep -q 'PK-21' docs/spec/pack-construction/README.md && ! grep -rqi 'fetched whole' docs/concepts/ && ! grep -q 'brings its neighbors along' docs/concepts/pack/README.md && grep -q 'parcel' docs/concepts/container/README.md && grep -q 'parcel' docs/concepts/storage-object/README.md && ! grep -rq 'rest of the Pack is as unfetched' docs/concepts/"
assignee: null
branch: task/1009-1234-fetch-a-container-by-fixed-aligned-parcels
created_at: 2026-10-09T12:34:10Z
updated_at: 2026-10-09T13:05:38Z
---

# docs(spec): fetch a Container by fixed, aligned parcels so the provider sees parcels rather than Entries

## Overview

PK-16 (`docs/spec/pack-construction/README.md`) makes the fetch unit a whole
Container and lets a client range-read the chunks covering one Entry as a
step inside fetching it. The reason given is privacy: the Storage provider
should learn no more of a reading pattern than which Container was read. The
implementation never finishes that step — `fetch_entry` reads exactly the
chunk run covering one Entry and stops, and the explorer's fill reads a
folder Entry by Entry — so what the provider actually observes is the
ciphertext byte range of every Entry read, in order: Entry boundaries and
sizes, which in a Pack of scanned books are volume and page sizes, and how
often each is revisited. Keeping the promise literally would mean fetching a
gigabyte Pack to show one page.

The decision: **the fetch unit becomes a fixed-length, aligned parcel of
the Container**, and the provider is promised the parcel as the
granularity it can observe. Rewrite the spec and the concept docs to say so.
Code is a separate task; this one changes documents only.

**Definitions to write down (spec, pack-construction).**

1. **Parcel.** A Container's chunk sequence (FM-2, FM-5) is divided from its
   first chunk into parcels of `n` consecutive chunks, where `n = max(1,
   S div chunk size)` and `S` is the spec constant below. The last parcel
   holds whatever chunks remain; a Container with no more than `n` chunks is
   one parcel. Parcel boundaries are positions in the chunk sequence and
   have nothing to do with where Entries begin or end. The front of the
   object — header and meta section (FM-2) — is not part of any parcel: it
   is read on its own, is the same read for every Entry of the Container,
   and names no Entry.
2. **Reads are issued by the parcel.** Every read of a Container's chunks
   asks for whole parcels: one parcel, several adjacent ones, or the whole
   object (every parcel at once, which `fetch_folders` does today). No range
   smaller than a parcel is issued, not even to show the first page early:
   the parcel is streamed, every chunk authenticates on its own (FM-5), and
   the Entry is released as soon as the chunks covering it have arrived. An
   Entry that spans a parcel boundary is reached by reading every parcel it
   overlaps. Keep the existing sub-bullet of PK-16 about holding an answer
   against the extent asked for.
3. **What the provider observes.** Rewrite the privacy claim: the provider
   can tell that parcel `k` of object `X` was read, and when; it cannot tell
   where an Entry begins or ends, nor how large one is, because no read
   starts or stops at an Entry. State the limit honestly: when `S` is close
   to the size of one volume, the provider can still tell volumes apart even
   though pages stay hidden. (This limit also belongs in the concept doc,
   see below.)
4. **`S` is a constant of the spec, provisionally 32 MiB**, with the
   sentence that its value is set by measuring how long the first page of a
   book takes to appear (`S` is both the privacy granularity and the lower
   bound on that wait: 32 MiB at 100 Mbit/s is under three seconds; 100 MiB
   would be about eight).
5. **A parcel on the device is not read again.** A fetched parcel is kept on
   the device until every Entry it covers is on the device or witnessed
   absent (EP-10, EP-11), and a parcel the device holds is never requested
   from Storage again, so a re-read of a page shows the provider nothing.
   Cancelling a fetch and reading ahead happen on parcel boundaries only.
6. **What is verified when only parcels were fetched.** An Entry taken out of
   parcels is verified by chunk authentication (FM-5, FM-7, FM-8) and by its
   plaintext hash against the catalog (EP-11). The Container's ciphertext
   hash (CP-11, CP-17) is verified only by a read of every parcel; a device
   that holds some parcels vouches for the Entries it placed and for nothing
   else in the Container. Say where that leaves CP-17's sentence that the
   hash is "what a reader verifies after fetching".

Give these rules ids. PK-16 keeps its id and is rewritten (its text is what
code cites today; the id is never renumbered). The rest are new ids after
PK-18 — at least `PK-19`, `PK-20` and `PK-21` must exist (a grep gate in
`check_command` pins the three ids). Tag each rule `*(Form: test)*` or
`*(Form: prose)*` as `docs/spec/README.md` prescribes; the normative tests
come with the code task and cite these ids, so rules whose content is
testable (alignment of reads, no re-read, what is verified) are `Form:
test`. Update the Mechanisms table in `docs/spec/README.md` if the
pack-construction row's "Covers" column no longer describes it.

**Concept docs.** The register's word for the unit is **parcel** — a plot
of land, a fixed piece at a fixed place — which does not collide with
anything the register already uses. Do not use
"segment": PK-3, PK-8, PK-18, `docs/concepts/README.md` and
`freeze/segment.rs` already use segmentation for a freeze cutting files
into Packs. Rewrite:

- `docs/concepts/container/README.md` — the Domain Rule "Fetched whole"
  becomes the parcel rule (name it so a reader finds it: "Fetched by the
  parcel"); introduce the term in that rule and in Collocations
  (`read (a parcel)`, `hold (a parcel on the device)`); keep "Streamable"
  and chunk run, and say that a parcel is a chunk run of fixed length at a
  fixed place.
- `docs/concepts/pack/README.md` — the Examples sentence "fetching one
  volume brings its neighbors along, which doubles as read-ahead" becomes
  what is true now: fetching one volume brings the parcels it overlaps, and
  with them whatever of the next volume shares those parcels; the
  Collocation `open` and the Domain Rule that ends "the rest of the Pack is
  as unfetched afterwards as it was before" are rewritten the same way
  (the rest of the Pack outside the parcels read is unfetched; the parcels
  read stay on the device). This also resolves the contradiction between
  that sentence and the Container doc.
- `docs/concepts/library/README.md` — Collocations `fetch` ("arrives by a
  range read over the chunks covering it alone"), `fill` and `supersede`
  ("stops between one Entry and the next" → between one parcel and the
  next).
- `docs/concepts/storage-object/README.md` — the residual-leakage list:
  the provider observes which parcels were read and when, not Entry
  boundaries; add the limit from rule 3 above.
- Any other concept or spec sentence that says the unit is the whole
  Container or an Entry (grep `docs/concepts docs/spec` for `whole
  Container`, `range read`, `range-read`, `neighbors`, `Entry by Entry`)
  and bring it in line.

Prose style: match the register — short rules with the reasoning as
sub-bullets; concept docs in the Definition / Domain Rules / Collocations /
Examples structure already there. Cite rules from concept docs as
`(spec: PK-19)`; inside the register bare `(PK-19)`. Do not reference any
document outside this repository.

Out of scope: code (format, usecase, server, tests); choosing the final
value of `S`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] PK-16 is rewritten to the parcel as the fetch unit, and new rules `PK-19`, `PK-20` and `PK-21` exist in `docs/spec/pack-construction/README.md` covering, between them, the parcel definition with `S`, reads issued by the parcel only, what the provider observes and its limit, retention and no re-read, and what is verified from parcels alone (grep gates on the ids appended to `check_command`)
- [x] No concept doc still says a Container is "fetched whole", that fetching a volume "brings its neighbors along", or that "the rest of the Pack is as unfetched" (grep gates appended to `check_command`), and the Container and Storage Object concepts use the word `parcel` (grep gates)
- [x] `make check` passes, including `spec-citations` and `spec-rule-ids` (every cited id has exactly one home)

### Before merge (verified outside the check command)

- [x] A read of `docs/spec/pack-construction/README.md` and the four concept docs in one sitting finds no sentence that still describes the Entry or the whole Container as the fetch unit — the merging session reads them and records what it checked
