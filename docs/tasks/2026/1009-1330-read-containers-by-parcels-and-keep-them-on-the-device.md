---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rqE "store\.get\([^)]*Some\(asked\)" backend/crates/domain/coffret-usecase/src/fetch/'
assignee: null
branch: task/1009-1330-read-containers-by-parcels-and-keep-them-on-the-device
created_at: 2026-10-09T13:23:20Z
updated_at: 2026-10-09T17:11:14Z
---

# feat(fetch): read Containers by fixed, aligned parcels, keep fetched parcels on the device, and fill a folder parcel by parcel

## Overview

The spec now makes the parcel the fetch unit (PK-16 and PK-19 to PK-22 in
`docs/spec/pack-construction/README.md`; the Container, Pack, Library and
Storage Object concept docs say the same): a Container's chunk sequence is
divided from its first chunk into parcels of `n = max(1, S div chunk size)`
chunks, `S` being a spec constant of 32 MiB for now; every read of chunks
asks for whole parcels (or the whole object); an Entry spanning parcels
reads every parcel it overlaps; a fetched parcel stays on the device until
every Entry it covers is on the device or witnessed absent, and is never
requested from Storage again; cancelling and reading ahead happen on parcel
boundaries; what parcels alone verify is chunk authentication and the
Entry's plaintext hash, not the Container's ciphertext hash. Read those
rules first; this task makes the code keep them.

Today `coffret-usecase/src/fetch/range_read.rs::read_entry` reads the front
(header + meta), then exactly the chunk run covering one Entry
(`outline.chunks_covering(entry.extent.range())`, fetched as
`store.get(object, Some(asked))`), and keeps nothing but the placed file.
`fetch/container.rs` (used by `fetch_folders`) reads whole objects.
`coffret-server/src/fill/run.rs` walks a folder's remote Entries one
`fetch_entry` at a time and checks `fills.superseded()` between Entries.

**1. Parcels in the format layer.** Beside `ChunkRun` give
`ContainerOutline` (`coffret-format/src/container_reader/`) the parcel
arithmetic: how many parcels the Container has, which parcel index a
plaintext offset falls in, the `ChunkRun` of parcel `k`, and the range of
parcel indexes an `EntryExtent` overlaps. The constant `S` lives where the
spec's other constants live (`coffret-model` or `coffret-format`; follow
the precedent of `ChunkSize::DEFAULT` and `DEFAULT_PACK_TARGET`), named so
a reader finds it from the spec (`PARCEL_LEN` or the like), cited `(spec:
PK-19)`. The front (header + meta) stays a read of its own as today.

**2. Fetch an Entry by its parcels.** `read_entry` computes the parcels the
Entry overlaps and, for each one this device does not already hold, issues
one `store.get` for exactly that parcel's ciphertext range (chunk-aligned,
`ChunkRun::ciphertext()`), streams it through `ChunkRunReader`, writes the
Entry's bytes to its scratch as it goes (so the first page of a book still
appears as soon as its chunks have arrived, not when the parcel ends), and
keeps the parcel's ciphertext on the device. A parcel already on the device
is decoded from there and Storage is not asked. The plaintext-hash
verification and the placement (`Placement::publish`) are unchanged
(EP-11). Whole-object reads in `fetch/container.rs` stay as they are: the
whole object is every parcel at once and is allowed by PK-16; its
ciphertext-hash check stays, since that path reads every chunk.

**3. Keep parcels on the device, and let them go.** PK-21 as written keeps a
parcel until every Entry with bytes in it is on the device or witnessed
absent, which never happens for an Entry this device does not map (the same
Pack holds a folder it never asked for), for the parcels of a Container a
later commit replaced or removed, and for a padding-only last parcel. Amend
PK-21 in this task so that a parcel is let go when every Entry it covers
*that this device maps* is present or witnessed absent, or when its
Container leaves the current set; cite the amended rule from the code. Say
the consequence in the concept docs too: the Index concept's Mental Model
table lists device-local records as mappings, materializations and pending
work — add held parcels there (a cache of ciphertext; losing them costs only
a re-read the provider can observe, never correctness), with a `hold` /
`let go` collocation if the Index is what records them; and the Mapping
concept's Domain Rules get one line that a device mapping only part of a
Pack still holds the parcels it read until their mapped Entries are placed.
Parcels need a home and a record. Put the ciphertext files under the state directory beside the
spool (a new capability in the local-fs gateway in the shape of `Spool`,
behind a port the usecase declares; follow the precedent of the spool
capability and the fetch destination) and record each kept parcel in the
Index as device state (SQLite: a new table keyed by Container id and parcel
index, with the layout version bumped and a migration in the style of the
`pending_rows` migrations; in-memory Index: the same; the `Index` trait
gains the methods to record, list and forget them; cite PK-21). A kept
parcel is removed, file and row together and idempotently (OC-8 is the
precedent for removals), when every Entry it covers is present on this
device or witnessed absent (EP-10, EP-11), and when its Container leaves
the current set (catch-up or a deletion this device committed). A parcel
file that is missing or does not authenticate is treated as not held: the
row is dropped and the parcel is fetched again, with a finding (the
`Findings` vocabulary in `coffret-device`) rather than a silent re-read.

**4. Fill a folder parcel by parcel.** `fill/run.rs` keeps its outcome
(Entries done / total, declined, degraded — the explorer's `fillLine`
and the work answer's `FillDto` do not change shape) but takes its steps in
parcels: for the next remote Entry it fetches the parcels that Entry still
needs; every other remote Entry of the folder that is now fully covered by
parcels on the device is placed from them without a Storage read; and
`fills.superseded()` is checked only between parcels, never inside one
(PK-21). Give the usecase whatever it needs for that — a way to place every
Entry a set of held parcels fully covers, and a hook the server can pass so
a read stops at a parcel boundary — rather than reimplementing parcel logic
in the server. `GET /api/file` (`routes/file.rs`) still fetches one Entry
on demand through `EntryFetches`; the explorer's reader prefetch of
neighbouring items (`prefetch.ts`) keeps asking per Entry and gets cheap
answers when the parcels are already held.

**5. Normative tests.** The `fetch_conformance` suite (`coffret-usecase/src/
fetch_conformance/`, run in memory and against MinIO) records every
`store.get` range in `CountingStore`. Add cases, each citing the rule it
holds:
- every range issued for a Container's chunks is exactly one parcel's
  ciphertext range or the whole object, never shorter (PK-16); build the
  fixture so a Pack has several parcels (set `S` small for the case, or
  make the parcel length a parameter the case can lower — say which);
- a Pack of several volumes: fetching one page reads the parcels it
  overlaps and nothing else, places the page, and the other volumes'
  Entries fully inside those parcels are placed too (PK-16, PK-19);
- an Entry across a parcel boundary reads both parcels (PK-16);
- a revisit — the same page again, or a neighbouring page inside a held
  parcel — reads nothing from Storage (PK-21);
- a parcel is let go once every Entry it covers is present, and kept while
  one is not (PK-21);
- a Container with only parcels fetched is not held to its ciphertext hash;
  a whole-object read still is (PK-22);
- a fill superseded mid-folder stops on a parcel boundary and the parcels
  read so far are still held (server route test in
  `coffret-server/tests/routes/fill.rs`, with the server's
  `counting_store`).
Update `partial.rs::one_entry_is_read_out_of_a_pack_without_reading_the_pack`
(its "the rest of the Pack is as unfetched" assertion flips to "outside the
parcels read") and every test or doc comment that says the unit is the
Entry's chunk run or the whole Container. The grep gate in `check_command`
pins that the old per-Entry `store.get(object, Some(asked))` read is gone
from `fetch/`.

**6. Explorer.** `fillLine` keeps counting Entries. If the per-Entry
`fetching` row state in `fill.ts` would now mark rows that are placed in
one stroke, make sure the listing refreshes so they turn present together
(check the existing refresh on `Placed`).

Naming: the unit is **parcel** everywhere (types, files, rows, findings,
doc comments) — never "segment", which the freeze already uses for cutting
files into Packs (PK-3). Vocabulary and citations follow the concept docs
and the spec; `make spec-citations spec-rule-ids` enforce the citation
form.

Out of scope: choosing the final `S` (measured later); resuming an
interrupted parcel read beyond "fetch it again"; a cap on how much parcel
ciphertext the device keeps (the retention rule bounds it to parcels with
an unplaced Entry); the CLI's `fetch` of whole folders beyond keeping it
spec-compliant.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Every range the fetch issues for a Container's chunks is one whole parcel or the whole object, and the per-Entry chunk-run read no longer exists in `coffret-usecase/src/fetch/` (conformance test; grep gate appended to `check_command`)
- [x] Fetching one page of a multi-volume Pack reads only the parcels it overlaps, places the page as soon as its chunks arrive, and places the other Entries those parcels fully cover; an Entry across a boundary reads both parcels (conformance tests)
- [x] A revisit of a held parcel reads nothing from Storage; a parcel is let go once every Entry it covers is present or witnessed absent and when its Container leaves the current set; a missing or unauthenticated parcel file is refetched with a finding (conformance tests, SQLite and in-memory Index tests including the layout migration)
- [x] A parcel-only fetch is not held to the Container's ciphertext hash and a whole-object fetch still is (conformance tests)
- [x] A fill superseded mid-folder stops on a parcel boundary, keeps the parcels read so far, and reports the Entries placed (route test); the work answer and the explorer's fill line keep their shape (contract fixtures unchanged or updated together with the frontend types)
- [x] `make check` passes

### Before merge (verified outside the check command)

- [ ] Needs a person: under `make desktop-dev` on the development Library, opening the first page of an unfetched scanned book shows the page without waiting for the whole Pack, turning pages inside the same parcel is immediate, and the server log shows parcel-sized reads only
