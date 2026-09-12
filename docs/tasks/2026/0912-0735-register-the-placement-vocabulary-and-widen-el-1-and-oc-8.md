---
status: completed
pipeline_phase: null
follow_up_of: null
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -qE "^- vouch \(for a mapped root" docs/concepts/library/README.md && grep -qE "^- vouch \(for itself, as the root" docs/concepts/library/README.md && grep -qE "^- vouch \(for what stands at a local path" docs/concepts/entry-path/README.md && grep -qE "^- refuse \(to place into a mapped root" docs/concepts/library/README.md && grep -qE "^- refuse \(a name or path that may not enter the Library" docs/concepts/entry-path/README.md && grep -qE "^- descend \(" docs/concepts/entry-path/README.md && grep -q "\*\*refused root\*\*" docs/concepts/library/README.md && grep -q "once for the mapping rather than once per" docs/concepts/library/README.md && grep -q "\*\*management area\*\*" docs/concepts/library/README.md && grep -q "reader decides from the name alone" docs/concepts/library/README.md && ! grep -qi "top-level prefix" docs/concepts/library/README.md && ! grep -qi "a prefix mapping" docs/concepts/library/README.md && grep -q "top-level component" docs/concepts/library/README.md && grep -q "component it stands for" docs/spec/event-logging/README.md && grep -q "provenance row" docs/spec/orphan-cleanup/README.md && grep -q "files and rows" docs/spec/orphan-cleanup/README.md && grep -q "files and rows a device wrote for itself" docs/spec/README.md && grep -q "staging directory" docs/spec/orphan-cleanup/README.md && grep -q "declines each" docs/spec/entry-path/README.md && grep -q "may go through more than one mapped root" docs/spec/entry-path/README.md && grep -q "the file a local writer fills" docs/spec/entry-path/README.md && ! grep -qi "the file a fetch writes before the rename" docs/spec/entry-path/README.md && grep -qE "^- scratch \(bytes a local writer" docs/concepts/library/README.md && grep -q "OC-8" docs/concepts/library/README.md && grep -q "no removal asks what is there" docs/concepts/library/README.md'
assignee: null
branch: task/0912-0735-register-the-placement-vocabulary-and-widen-el-1-and-oc-8
created_at: 2026-09-12T07:35:18Z
updated_at: 2026-09-12T16:00:14Z
---

# docs: register the vocabulary the placement rules use, and widen EL-1 and OC-8

## Overview

The rules that decide what a device may place into a mapped folder are stated
— EP-11 for the placement itself, EP-12 for whether a root is there to be read
from, EP-13 for whether the folder standing there is the one that was
registered, EP-14 for the name the device keeps for itself — and the code now
leans on words those rules use that the concept documents never register, and
cites three rules whose text is narrower than what it does.

Thirteen things to settle, in two places, on one line:

- **The concept documents own meaning, promises and vocabulary.** A word the
  code and the rules both use, a verdict a person is told about, a cost a
  person bears: these belong in a concept document's Collocations or Domain
  Rules.
- **The register owns behaviour and format mechanics cited by ID.** Which
  readers apply a reservation, at what depth, what a marker's bytes are, what a
  diagnostic event may retain, what a removal is idempotent over: these belong
  in a rule, and a concept document cites the rule rather than restating it.

That line puts eight of the items in `docs/concepts/`, four in `docs/spec/`,
one in both, and leaves one out entirely because the register already answers
it (see **Out of scope**).

Six files:

- `docs/concepts/library/README.md`
- `docs/concepts/entry-path/README.md`
- `docs/spec/entry-path/README.md`
- `docs/spec/event-logging/README.md`
- `docs/spec/orphan-cleanup/README.md`
- `docs/spec/README.md`

Every line number below is a line number in this task's base ref. Re-read the
neighbourhood before editing rather than trusting the number, since the ref
moves.

Both concept documents are cold-reader documents. Read each top to bottom
before editing it, keep the sentence rhythm of the neighbours of every line you
touch, and wrap at the document's existing width — 79 columns, which the
existing lines respect once an em-dash is counted as the one character it is.

### Editing a rule's text in place

Three of these sections change the text of a rule that already exists, and one
more adds sub-bullets to a rule without touching the text it has. That is
allowed here and has been done before: `docs/spec/entry-path/README.md`'s EP-8
was rewritten from three lines into ten under the same ID, keeping its
`*(Form: test)*` clause and gaining a sub-bullet, when the read and write
confinement boundary was settled. The rules that follow it kept their numbers.
So:

- A rule ID is never renumbered and never reused. EL-1 stays EL-1, OC-8 stays
  OC-8, and EP-11 stays EP-11.
- Every rule carries a `(Form: …)` obligation and keeps the one it has. EL-1's
  is `prose` with its own parenthetical reasoning; OC-8's and EP-11's are
  `test`. None of them changes here, because neither the kind of evidence nor
  the reason for it changes.
- **Widening needs its own justification.** EL-1 is a permission, OC-8 is an
  enumeration of what a removal covers, and EP-11's scratch sub-bullet scopes a
  name reservation; widening any one of them licenses something an
  implementation could not do before. So each of those sections states what the
  code already relies on, and the widening goes no further than that. All three
  widen the *person-facing*, *local-bookkeeping*, or *local-writer* side. None
  lets one more byte into a diagnostic event, and none touches what a removal
  may reach on Storage.

## 1. Register *vouch*, with both of its subjects

The verb carries the whole of EP-11, EP-12 and EP-13 in the code, across the
fetch, sync, freeze, scan and placement modules and the server routes over
them, and no Collocations list has it. The Library concept already uses it in
prose (`docs/concepts/library/README.md`
line 89: *"A mapped root this device cannot vouch for … is an **unavailable
root**"*), and so does the Entry Path concept (line 82: *"A fetch **places** an
Entry only where this device can vouch for what is at the path"*).

Two subjects, kept apart deliberately:

- the **device** vouches for a mapped root — whether the root is there to be
  read from (EP-12);
- the **root** vouches for itself — whether the folder standing there is the
  one whose marker the mapping recorded (EP-13).

Both of those are about mapped roots, which is the Library concept's ground: it
is where **unavailable root** is defined, where **materialize** is defined, and
where the filesystem identity a scan **stamps** is registered. So the Library
concept owns *vouch*, and it states both subjects. The list already carries two
`stamp` entries that differ only in subject and object, so one verb twice is
this list's own form.

The third pairing — the device vouching for what stands at a local path, which
is EP-11 and the sense most of the fetch code is written in — belongs to the
Entry Path concept, whose Collocations already own the path-side verdicts
`place` and `decline`, and whose Domain Rules already use the verb.

**`docs/concepts/library/README.md`, Collocations.** Insert after the second
`stamp` entry (*"stamp (a fetched file with its Entry's own modification
time)"*) and before `surface`:

```
- vouch (for a mapped root, as the device — whether the root is there to be
  read from)
- vouch (for itself, as the root — whether the folder standing there is the one
  whose marker the mapping recorded)
```

**`docs/concepts/entry-path/README.md`, Collocations.** Insert after
`decline`:

```
- vouch (for what stands at a local path, as the device, before a fetch places
  an Entry there)
```

## 2. Name the EP-13 verdict a **refused root**

Its sibling is named in bold one sub-bullet above it — *"A mapped root this
device cannot vouch for … is an **unavailable root**"* — while the EP-13 case
is given by predicate only (`docs/concepts/library/README.md` lines 95–99: *"A
root is also refused for placement when its marker is absent or does not
carry…"*). The code names it everywhere: `RefusedRoot` in
`coffret-usecase/src/refused_root.rs`, `Finding::RefusedRoot` in
`coffret-device/src/finding.rs`, `reason: "refused_root"` on the wire in
`coffret-server/src/api_error/mod.rs`, and *"refused root …"* in the line a
person reads at a terminal (`coffret-device/src/finding.rs`, the `Display` arm).

Define it in the same shape and the same place as its sibling.

**`docs/concepts/library/README.md`.** Replace the sub-bullet at lines 95–99
with:

```
  - A mapped root that will not vouch for itself — its marker absent, or
    carrying an identity other than the one recorded for that mapping at
    registration — is a **refused root**. Nothing is placed into it and the run
    reports the mapping, so a disk that came back empty or a folder that
    merely answers to the registered name is never written into. The check is
    separate from availability and is made before a fetch, an upload, or a sync
    writes anything: an available root can still be the wrong folder
    (spec: EP-13).
```

Naming it has one consequence in the same document. The finding sub-bullet at
lines 173–175 says an unavailable root is a finding about a mapping rather than
a file; a refused root is the other one, and leaving it unsaid directly under
the new name would read as though only one of the two is reported. Extend that
sub-bullet to:

```
  - An unavailable root is a finding of the same kind, about a mapping rather
    than a file, so a successful run carrying one has scanned less of the
    Library than this device's mappings cover (spec: EP-12, PK-14). A refused
    root is reported the same way, once for the mapping rather than once per
    Entry, so a run carrying one has placed less than its mappings cover
    (spec: EP-13, PK-14).
```

## 3. State the read side of the management area, and define the phrase

Two items meet here, so they are one edit.

What the concept documents say today about a reader meeting the device's own
folder is: nothing. `docs/concepts/entry-path/README.md` mentions *"the root's
management area"* at line 63 only as where the marker lives, and at line 95
only to refuse a placement into it. The phrase is never defined anywhere in
`docs/concepts/`. The Library concept, meanwhile, states the reserved *prefix*
in full — what a fetch writes under it, that a scan passes over it, and what
that costs (lines 135–142) — and says nothing at all about the reserved *name*,
even though the register states both costs, symmetrically, in EP-11 and EP-14.

So the promise belongs in the Library concept, beside the prefix whose promise
it mirrors, and the phrase gets its definition there — a concept document is
where a word is defined, and EP-14 defines the *name* rather than the thing.
The mechanics stay the register's: which readers apply the reservation, that it
holds at any depth, and that an inner root's area is not the outer root's
content are EP-14's to say, and the concept cites it.

Do not name the literal `.coffret` here. Neither concept document names it
today — `docs/concepts/entry-path/README.md` line 95 says *"The name reserved
for the device's own management area"* — and the literal is EP-14's.

**`docs/concepts/library/README.md`.** Add as a new Domain Rule immediately
after the fetch/scratch rule and its cost sub-bullet (after line 142), before
the restore rule:

```
- The device also keeps a **management area** inside each mapped root — a
  folder holding what the device records about that root rather than any of the
  Library's content, the root's own marker among it. The name is reserved at
  any depth under a mapped root and every reader decides from the name alone: a
  scan never enters it, a listing of a mapped folder leaves it out, and nothing
  is ever placed at a path carrying it (spec: EP-13, EP-14).
  - The cost is the one the reserved prefix above carries: anything of the
    user's own under a folder of that name is not backed up, since a reader
    stops at the name and never looks inside.
```

The three readers named there are the three the code has:
`local_scan/walk_mappings.rs` and `local_scan/root_state.rs` step over the name
as they walk, `coffret-device/src/add/added_locally.rs` leaves it out of a
mapped folder's listing, and `fetch/select.rs` with
`coffret-device/src/add/receive_file.rs` refuse a path carrying it.

## 4. Register *refuse* beside *decline*, and say what separates them

They are not one thing under two words. What separates them is what is being
turned away:

- **decline** is the verdict a placement reaches about **one Entry**: *"Every
  Entry a fetch declines to place is reported with the reason it was declined"*
  (EP-11), and the Entry Path concept registers it that way.
- **refuse** is the verdict on something that is not one Entry: a root
  (EP-13's *"The device **refuses to place** when the root is missing…"*), a
  path arriving from outside the Library that is malformed or misshapen (EP-1,
  EP-2), a final name whose opened handle is not a regular file (EP-8), and a
  reserved name (EP-14).

The documents already reconcile the overlap rather than confuse it:
`docs/concepts/entry-path/README.md` line 96 says the EP-14 refusal *"is
reported like any other declined Entry"*, and EP-13 says a refusal *"propagates
the way a declined placement does"*. So both words stay, each registered where
its object lives, with the distinction stated in the entry itself.

**`docs/concepts/entry-path/README.md`, Collocations.** Insert after the new
`vouch` entry from section 1:

```
- refuse (a name or path that may not enter the Library at all) — the wider
  verdict beside decline: malformed, unspellable, or carrying a reserved name,
  where a decline is what one placement says about one Entry
```

**`docs/concepts/library/README.md`, Collocations.** Insert after the two new
`vouch` entries from section 1:

```
- refuse (to place into a mapped root that will not vouch for itself)
```

## 5. Register *descend*

`docs/concepts/entry-path/README.md`'s Domain Rules use the verb at line 58 —
*"scans, source reads, served files, and fetch writes descend validated
relative components without following links"* — EP-8 says *"descended"* and
EP-13 *"descends"*, and no Collocations list has it. The code carries it as a
type (`coffret-usecase/src/descent_error.rs`) and, in the redacted event
grammar, as a field whose two values are pinned by tests: `descent=blocked` and
`descent=unspellable` appear in `coffret-model/src/redacted.rs`,
`fetch/fetch_error.rs`, `coffret-device/src/error.rs`,
`coffret-server/src/api_error/tests.rs` and `coffret-server/tests/routes.rs`.

It is the Entry Path concept's: it is how a translated path is actually reached,
and that list already owns `translate` and `place`.

**`docs/concepts/entry-path/README.md`, Collocations.** Insert between
`translate` and `place`, so the list reads as the order the acts happen in:

```
- descend (validated relative components from an open mapped root, without
  following links)
```

## 6. Let EL-1's permission cover a mapping

`docs/spec/event-logging/README.md` EL-1 forbids an Entry Path and a local path
in a diagnostic event, then permits: *"A person-facing refusal may identify a
file or Library that person owns."* A mapping is neither, and identifying one
is now relied on in two directions:

EP-13 requires it. Its refusal bullet says *"A refusal names the mapping and
the reason"*, and a mapping is named the two ways EP-9 gives a reader: by the
top-level component it stands for, or by the folder on this device it is rooted
at. Both spellings are in use:

- The refused-root sentence a browser is answered with, in
  `coffret-server/src/api_error/mod.rs`, argues the EL-1 question in its own
  doc comment rather than citing a clause that settles it: a local path does
  not cross that boundary, while the Library-side component does, *because a
  name inside the Library is the person's own and a path on this device is
  not*. That argument is load-bearing — without it the sentence cannot name
  which mapping to aim `coffret map` at — and it is made at the call site
  because EL-1's permission does not reach it.
- `coffret-device/src/error.rs`, the `MalformedMappingPrefix` arms, put the
  top-level component the person typed into the refusal: *"a mapping stands for
  one top-level component of the Library, and this names more than one"*. And
  `Finding::RefusedRoot`'s `Display` names the local folder to whoever is at a
  terminal.

Why the widening is safe, and why it is not larger than it has to be: the
sentence being changed is the one that says what a **person** may be told, and
nothing else in EL-1 moves. The forbidden list is untouched, the enforcement
clause is untouched, and the `(Form: prose …)` obligation is untouched. What is
added is the identification the code already makes, by the two spellings it
already makes it in, and the closing *"that rendering is not reused for an
event"* keeps both of them out of the log exactly as before — which is what
`Noted` in `coffret-server/src/noted.rs` and `RefusedRoot`'s own doc comment
already implement.

**`docs/spec/event-logging/README.md`.** Replace EL-1 with, wrapped exactly
like this so the permission clause stays readable:

```
- **EL-1.** A diagnostic event must not contain an Entry Path, a local path or
  filename, a device-local Library name, plaintext file content, a
  cryptographic key, a Passphrase, a Recovery Code, or a token or other bearer
  credential. A person-facing refusal may identify a file, a Library, or a
  mapping that person owns — a mapping by the top-level component it stands for
  (EP-9), or by the local folder it names; that rendering is not reused for an
  event. This is enforced by constructing event fields from log-safe facts and
  `Redacted` renderings rather than `Display`. *(Form: prose — absence across
  every event site is a review and construction obligation; regression tests
  cover concrete boundaries.)*
```

Keep *"component it stands for"* whole on one line, as it is wrapped above: a
gate below reads it as one line.

## 7. Let OC-8's enumeration name the provenance row and the staging directory

`docs/spec/orphan-cleanup/README.md` OC-8 makes idempotent the removal of *"a
local file this device wrote for its own purposes — a spool file, or a scratch
a fetch never published (EP-11)"*. The catalog the device keeps for its own
bookkeeping is such a file, so the subject clause reaches the rows in it; the
examples name only files, so nothing in the rule says so.

What that costs today is visible: dropping a provenance row is idempotent for
exactly OC-8's reason, and `coffret-usecase/src/index.rs` cites OC-6 for it —
*"Dropping one that is not there succeeds, so an interrupted cleanup is simply
run again (spec: OC-6)"* — as does
`index_conformance/device_state.rs` for marking an already-`Spooled` row. OC-6
is a claim about a Container's object on the provider's trash, and OC-8's own
sub-bullet says why it is not the rule for a device's own leftovers: *"a
device's own leftovers are not the Library's, so their removals need a rule of
their own and do not widen OC-6."* OC-8 is what the spool's own removals
already cite — `coffret-usecase/src/spool.rs` and
`spool_conformance/removal.rs` — while the row's citations still point at OC-6,
and a code change that moves those should find the rows named in the rule it
moves them to.

The enumeration is short by a second member, and for the same reason: what
this device wrote for its own purposes is not always a file either.
`backend/crates/apps/coffret-device/src/staging.rs` builds a Library in a
directory named after neither name it might end up with, and removes that
directory — `remove_dir_all`, recorded as the same `LocalOperation::Removing` a
spool's and a scratch's removals are — both when an interrupted earlier attempt
left one standing and when the attempt it is running gives up. That is neither
a spool file nor a scratch, and a directory is not a file, so the rule's own
subject clause has to stop saying *a local file* for the enumeration to reach
it. The idempotence is already what that code relies on: an absent staging
directory is nothing to remove rather than a failure, and the removal is never
conditioned on what the directory holds, since nothing in it reached Storage
under a key anything kept.

Both members go into one replacement: it is one edit to one enumeration, and
the subject clause each of them needs widened is the same one.

Why the widening is safe: it adds the row and the directory to the enumeration
of a rule whose subject already covers them once it stops saying *a local
file*, and it adds nothing on the Storage side — OC-6's scope is untouched, and
so is the sub-bullet that keeps the two apart. The `*(Form: test)*` obligation
is unchanged, because a row's and a directory's idempotent removal is as
testable as a file's.

**`docs/spec/orphan-cleanup/README.md`.** Replace OC-8's own bullet (its
sub-bullet below it is unchanged) with:

```
- **OC-8.** Removing what this device wrote for its own purposes — a spool
  file, a scratch a fetch never published (EP-11), the staging directory an
  interrupted attempt at putting a Library on this device left, or the
  provenance row (OC-2) in the device's own catalog that announced the spool —
  is idempotent. What is already gone is a successful removal, an interrupted
  clean-up is simply run again, and no removal ever checks what is there first,
  because absence is the outcome being sought. *(Form: test)*
```

Keep *"staging directory"* and *"provenance row"* each whole on one line: a
gate below reads each of them as one line.

Two summaries name what OC-8 covers and both now say one word too little.

**`docs/spec/orphan-cleanup/README.md`**, the mechanism's own opening
paragraph, lines 5–7 — replace with:

```
be proven. The provenance a cleanup rests on can also prove the opposite —
that the batch did commit — and what that obliges instead is here too, along
with the idempotence of removing the local files and rows a device wrote for
itself.
```

**`docs/spec/README.md`**, the Mechanisms table, the Orphan Cleanup row: change
*"the idempotence of removing the local files a device wrote for itself"* to
*"the idempotence of removing the local files and rows a device wrote for
itself"*. That row is one long line; leave it one long line.

Both summaries stop at *files and rows* deliberately. They say which kinds of
thing a reader will find the rule about, and a staging directory is a tree of
this device's own local files; the enumeration inside the rule is the place
that names each member.

## 8. Settle on *top-level component* in the Library concept

`docs/concepts/library/README.md` says *"top-level prefixes"* (line 26) and
*"top-level prefix"*, *"each prefix"*, *"a prefix mapping"* (lines 78–80). EP-9
says *"top-level Entry Path component"*, *"each top-level component"* and *"a
top-level mapping"* throughout, and EP-1's own boundary paragraph distinguishes
the two words in one sentence: *"the top-level component a device's mapping is
configured with, a prefix a caller narrows a run to"*.

The concept document should follow the register, and the reason is stronger
than consistency: *prefix* is already taken twice over inside this very
document. Line 137 says *"coffret reserves a local filename prefix for those
files"* — EP-11's reserved local filename prefix, a different thing entirely —
and line 57's `scratch` collocation says *"under the reserved prefix"*. A
reader meeting *prefix* in the Library concept today cannot tell which of the
two is meant from the word. `component` is unambiguous; *prefix* then means one
thing in this document, which is EP-11's.

The code's field is named `prefix`
(`coffret-usecase/src/device_state/mapping.rs`). Renaming it is a code change
and is out of scope here.

**`docs/concepts/library/README.md`.** Two edits, both rewrapped.

Lines 25–32, the paragraph opening *"One or more local folders form that
working view"* — given whole, because the word change moves every line break
in its first half:

```
One or more local folders form that working view. A device can map one folder
to the Library root, map folders to top-level components, or combine both —
for example, keeping most of the Library on one disk and `albums/` on another.
The Library root is the root of the [Entry Path](../entry-path/) namespace; it
does not have to correspond to one folder on disk. Every
[Entry](../container/entry/) records its Entry Path relative to that root and
never a device path, so one Library restores onto whatever arrangement of
disks a device happens to have.
```

Lines 78–83, the mapping Domain Rule:

```
- A local folder maps either to the Library root or to a top-level component
  of the Entry Path namespace. A device may have at most one root mapping, and
  each top-level component maps to at most one folder. When both are present,
  a top-level mapping represents that part of the Library and the root mapping
  represents the rest. These mappings belong to the device, so another device
  may arrange the same Library differently (spec: EP-9).
```

The word *prefix* stays in line 57's `scratch` collocation and in line 137's
*"local filename prefix"*. Those are EP-11's prefix and are correct — section
10 rewords that collocation for an unrelated reason and keeps the word as it
is.

## 9. EP-11's single-writer gloss does not contemplate several placements

`docs/spec/entry-path/README.md`'s EP-11 sub-bullet reads:

> A folder fetch continues past an Entry it declines and reports each one, while a single writer — the upload
> route the browser drops a file into, or a write already in progress — fails as a whole when its **one
> placement** is declined.

The upload route is named as a single writer, and a multipart drop hands it several placements rather than
one. The route already treats the two sides differently and has to: a part refused for its own name is
reported beside what landed, while a refusal about the mapped root every part of a folder drop goes through
fails the request as a whole (EP-13). If "a single writer" were categorical, every refusal on that route would
fail the whole request, which the route's own per-part refusals contradict — so the distinction is real and
the rule does not state it.

Add a sub-bullet under EP-11, after the one quoted above:

```
  - A single writer handed several placements at once — the upload route's multipart drop — declines each
    placement that is one file's business and reports it beside what it placed, and fails the request as a
    whole only where the refusal is a mapping's business rather than a file's (EP-13). Which side a refusal
    falls on is the condition it stands on, not the wire kind it is answered with.
```

EP-9 makes the interaction sharper and the sub-bullet should not paper over it: a device may hold a
Library-root mapping *and* a top-level mapping, and a drop addressed at the Library root resolves a mapping
per part. There, a refusal about one mapping stops parts bound for another. State that consequence rather
than leaving a reader to find it:

```
  - Where a drop is addressed at the Library root and the device holds more than one mapping (EP-9), its parts
    may go through more than one mapped root, and the first refusal that is a mapping's business ends the
    request — including for parts a sound mapping would have taken.
```

This is the one item here whose subject is behaviour a person meets rather than vocabulary, so it is the one
to read hardest: the sentence must describe what the route does, not what it ought to do.

## 10. Widen EP-11's scratch reservation to the writer that shares it

EP-11 scopes the scratch to a fetch — *"A **scratch** is the file a fetch
writes before the rename that publishes it"* — and scopes the reservation the
same way one clause later: *"a fetch gives its scratches no other kind of
name"*. A second writer shares that prefix, and the code says so from both
sides — the writers' and the scan's.

`backend/crates/domain/coffret-usecase/src/scratch.rs` opens with *"One prefix,
shared by everything that writes into a folder a scan walks"*, and its `PREFIX`
doc says *"The reservation serves a second writer as well as the fetch: a file
arriving from outside — the explorer taking a dropped file into a mapped folder
— … so it takes its scratch names from here too"*. That writer is
`scratch::incoming_name()`, called from
`coffret-device/src/add/incoming_file.rs`. And the reader the reservation
exists for cannot tell the two writers apart: `local_scan/walk_mappings.rs`
steps over a name through `scratch::is_scratch`, which asks whether the name
starts with the prefix and nothing else.

So the register describes a narrower reservation than the one the scan relies
on. Two things follow, and the second is what keeps the widening small:

- the **reservation** is a local writer's rather than a fetch's, because the
  prefix is what every writer publishing by rename into a mapped folder takes
  its names from, and because the promise a scan makes is about the name and
  not about who wrote it;
- the sentence about a **fetch** stays a sentence about a fetch. *"A fetch
  gives its scratches no other kind of name"* says that one writer uses no
  other spelling, which is still true and still worth saying — deleting it
  would lose the exhaustiveness the crash argument rests on. The same is then
  said of the second writer.

Nothing else in EP-11 moves. The bullet below the sub-bullet, which gives the
literal prefix and says a scan decides from the name alone, is already
writer-neutral and is left as it is.

**`docs/spec/entry-path/README.md`.** The scratch sub-bullet under EP-11 reads:

> A **scratch** is the file a fetch writes before the rename that publishes it.
> It is written inside a mapped folder, which is also a folder a scan walks, so
> coffret reserves a local filename prefix for it: a fetch gives its scratches
> no other kind of name, and a scan passes over every local name carrying that
> prefix instead of reporting it as a file to back up (EP-1, EP-8). A run
> killed between the write and the rename therefore leaves nothing a later sync
> would commit as an Entry. The cost is that anything of the user's own
> carrying that prefix is not backed up — a file, or a folder and everything
> under it, since the scan stops at the name and never looks inside — which is
> the trade for a crash never inventing an Entry out of a partial fetch.

Replace it with:

```
  - A **scratch** is the file a local writer fills before the rename that
    publishes it. It is written inside a mapped folder, which is also a folder
    a scan walks, so coffret reserves a local filename prefix for it. Every
    local writer that publishes by rename takes its scratch names from that
    prefix, and a scan passes over every local name carrying it instead of
    reporting it as a file to back up (EP-1, EP-8). A fetch gives its scratches
    no other kind of name, and neither does an upload the browser drops into a
    mapped folder, which is written and renamed for the same reason. A run
    killed between the write and the rename therefore leaves nothing a later
    sync would commit as an Entry. The cost is that anything of the user's own
    carrying that prefix is not backed up — a file, or a folder and everything
    under it, since the scan stops at the name and never looks inside — which
    is the trade for a crash never inventing an Entry out of a partial fetch.
```

Keep *"the file a local writer fills"* whole on one line, as it is wrapped
above: a gate below reads it as one line.

**`docs/concepts/library/README.md`, Collocations.** The `scratch` entry at
lines 57–58 scopes the act the same way — *"scratch (a fetched Entry's bytes to
a name under the reserved prefix before the rename that publishes them)"* — and
a concept document is where an act's subject is registered, so it follows the
rule. Replace those two lines with:

```
- scratch (bytes a local writer puts under the reserved prefix before the
  rename that publishes them — a fetched Entry's, or a file taken into a mapped
  folder from outside the Library)
```

## 11. Cite OC-8 from the concept documents, as the Storage side already does

The Storage side of removal has a way in from the concept documents.
`docs/concepts/storage-object/README.md` names the case and cites the rules
that make it idempotent: *"Such a Container is an **untrashed removal**: any
later run may trash it, and doing so is idempotent, because the record rather
than an inference is what proves the removal (spec: OC-6, CP-14)."* The local
side has no such sentence. The Library concept defines **spool** and
**scratch**, and the Index concept's Collocations carry *"dispose (an
interrupted run's spool, the object if one was uploaded, and the pending row
naming them)"* — the act OC-8 makes idempotent — but nothing under
`docs/concepts/` cites OC-8 at all. A reader who comes to a device's own
leftovers from the concept documents reaches OC-6 and never the rule their
removal actually rests on.

It goes in the Library concept rather than beside `dispose`, for two reasons.
The Storage side's citation sits in a Domain Rule and not in a Collocations
entry, and the Index concept's Collocations carry no rule IDs at all, so one
there would be the only citation in that list. And `dispose` spans the uploaded
object as well as the local leftovers — the object is OC-6's half of the pair
OC-8's own sub-bullet keeps apart, so a citation on that entry would read as
one rule covering both. The Library concept's sync rule is where the local half
is already described, in the settle clause that says what an interrupted run
leaves for the next one.

**`docs/concepts/library/README.md`.** The sync-stages Domain Rule (lines
130–134) ends *"Everything before it is device-local work that an interrupted
run leaves behind for the next one to settle (spec: CP-1, OC-2, OC-7)"* and
carries no sub-bullet. Add one:

```
  - Whatever of that work a settle reclaims rather than completes is removed,
    and each removal is idempotent: an interrupted settle is simply run again,
    and absence is the outcome sought, so no removal asks what is there before
    it removes (spec: OC-8).
```

Keep *"no removal asks what is there"* whole on one line, as it is wrapped
above: a gate below reads it as one line.

That states the promise and cites the rule rather than restating what the rule
enumerates: which leftovers OC-8 covers is the register's, and the concept's
own **spool** and **scratch** entries are what a reader follows from here.

## Acceptance criteria

`make check` does not read prose, so every gate below is a `grep` over the
documents. They prove that a line is there, not that it says the right thing:
whether the definitions read well to a cold reader, whether the three widened
rules still mean what they meant, and whether each new Collocations entry is
the sentence its neighbours are in, is the author's judgement and is what the
review rounds are for. Each gate was run against `feat/mapped-root-marker`
before being written down, and none of them passes there.

### Automated (pipeline-verified)

- [x] Existing backend, frontend, and interoperability checks continue to pass:
      `make check`.
- [x] The Library concept registers *vouch* with both of its subjects:
      `grep -qE "^- vouch \(for a mapped root" docs/concepts/library/README.md`
      and
      `grep -qE "^- vouch \(for itself, as the root" docs/concepts/library/README.md`.
- [x] The Entry Path concept registers the placement sense of *vouch*:
      `grep -qE "^- vouch \(for what stands at a local path" docs/concepts/entry-path/README.md`.
- [x] Both concepts register *refuse* beside the verdict it is not:
      `grep -qE "^- refuse \(to place into a mapped root" docs/concepts/library/README.md`
      and
      `grep -qE "^- refuse \(a name or path that may not enter the Library" docs/concepts/entry-path/README.md`.
- [x] The Entry Path concept registers *descend*:
      `grep -qE "^- descend \(" docs/concepts/entry-path/README.md`.
- [x] The EP-13 verdict is named, and reported like its sibling:
      `grep -q "\*\*refused root\*\*" docs/concepts/library/README.md`
      and
      `grep -q "once for the mapping rather than once per" docs/concepts/library/README.md`.
- [x] The Library concept defines the management area and states that a reader
      decides it by name:
      `grep -q "\*\*management area\*\*" docs/concepts/library/README.md`
      and
      `grep -q "reader decides from the name alone" docs/concepts/library/README.md`.
- [x] The Library concept keys a mapping by a component and no longer by a
      prefix:
      `grep -q "top-level component" docs/concepts/library/README.md`,
      `! grep -qi "top-level prefix" docs/concepts/library/README.md`,
      and `! grep -qi "a prefix mapping" docs/concepts/library/README.md`.
- [x] EL-1 permits identifying a mapping by what it stands for:
      `grep -q "component it stands for" docs/spec/event-logging/README.md`.
- [x] OC-8 names the provenance row, and both summaries of it follow:
      `grep -q "provenance row" docs/spec/orphan-cleanup/README.md`,
      `grep -q "files and rows" docs/spec/orphan-cleanup/README.md`,
      and `grep -q "files and rows a device wrote for itself" docs/spec/README.md`.
- [x] and it names the staging directory an interrupted create or join left:
      `grep -q "staging directory" docs/spec/orphan-cleanup/README.md`.
- [x] EP-11 says what a single writer handed several placements at once does,
      rather than leaving "its one placement" to cover a multipart drop:
      `grep -q "declines each" docs/spec/entry-path/README.md`.
- [x] and it says what a drop addressed at the Library root does on a device
      holding more than one mapping:
      `grep -q "may go through more than one mapped root" docs/spec/entry-path/README.md`.
- [x] EP-11's scratch reservation is a local writer's rather than a fetch's,
      and the Library concept's `scratch` entry follows it:
      `grep -q "the file a local writer fills" docs/spec/entry-path/README.md`,
      `! grep -qi "the file a fetch writes before the rename" docs/spec/entry-path/README.md`,
      and
      `grep -qE "^- scratch \(bytes a local writer" docs/concepts/library/README.md`.
- [x] The Library concept cites OC-8 for the removal of what an interrupted run
      left, so the local side of removal has a way in from the concept
      documents:
      `grep -q "OC-8" docs/concepts/library/README.md`
      and
      `grep -q "no removal asks what is there" docs/concepts/library/README.md`.

## Out of scope

**Giving *Place* as a noun a documented home.** The claim does not hold: EP-11
already defines it, in bold, in `docs/spec/entry-path/README.md` — *"A
**place** is the local path a fetch resolves an Entry to, so an unreachable
place and placing an Entry are one word seen twice"* — which is exactly what
`LocalPlace`, `Surfaced::UnreachablePlace` and *"an empty place"* in
`coffret-usecase/src/destinations.rs` are named after, and the verb side is
already registered as `place` in the Entry Path concept's Collocations. There
is nothing to add, and the register is the right place for it: it is the
definition a rule's own mechanics rest on, not a promise to a person.

**A separate concept-document entry for "management area".** The phrase does
need defining, and the definition lands in section 3 rather than in an entry of
its own — it is the same edit, and splitting it would put a word's definition in
one place and the promise that gives it a reason in another.

**Any code change.** No Rust or TypeScript file is touched. The `prefix` field
on a mapping keeps its name; the doc comments in
`coffret-usecase/src/index.rs` and `index_conformance/device_state.rs` keep
citing OC-6 until a code change moves them to OC-8, whose enumeration section 7
widens to reach the rows rather than performing that move. `scratch.rs` keeps
its doc comments as they are too: section 10 widens the rule to the reservation
they already describe, and nothing there has to change to say it.

**Any other rule's text.** EP-12 through EP-14 are correct as they stand and
are only cited from the concept documents here. EP-11 is the exception: section
9 gives it two sub-bullets it does not have and section 10 restates its scratch
sub-bullet, and nothing else in it moves.
