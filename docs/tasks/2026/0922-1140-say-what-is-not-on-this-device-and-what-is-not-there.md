---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [user-experience, completeness, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/0922-1140-say-what-is-not-on-this-device-and-what-is-not-there
created_at: 2026-09-22T02:40:00Z
updated_at: 2026-09-22T05:55:00Z
---

# feat(explorer): say what is not on this device, and what is not there

## Overview

Five places where the explorer and the route behind it show a person a folder
without saying which of two very different things is true of it: that this
device has nowhere to put what the folder holds, or that the folder is not
there at all. Each was found in use.

The wire already carries the first of the two. `ListingDto.mapped` and each
`FolderDto.mapped` say whether a folder on this device stands for that subtree,
the rows carry a `not here` chip, and an unmapped folder puts a banner over its
rows. So this change is not about learning the fact — it is about the places
that have the fact and still leave a person guessing, and the one place where
the fact is missing entirely.

### 1. A file row in an unmapped folder answers nothing at all to a click

`FileList.tsx` passes `onActivate={undefined}` for every file row when
`listing.mapped` is false. The reasoning beside it is sound — a fetch under an
unmapped folder would be declined, so the row is shown and not offered rather
than walking a reader into a refusal — but the result is a row that a person
clicks and clicks and that never once answers.

The same component already knows the better shape. A **drop** onto an unmapped
folder does not silently do nothing: it calls `onUnmapped`, and `App.tsx`
answers with a notice saying nothing was added and why. The comment there
states the rule this change should carry to the click — "a drop is a gesture
with an outcome, and the outcome here is that none of those files were added —
which a screen that goes on looking exactly as it did does not tell anybody".

A click is a gesture with an outcome too. Give it the same answer. Keep the row
un-openable; what changes is that the attempt is met with a sentence rather
than with nothing. Read `onUnmapped`'s notice first and decide whether the two
gestures say the same sentence or two sentences that differ in what was tried —
either is defensible, and say in a comment which you chose and why.

### 2. The banner tells a folder with no files how to fetch its files

`Unmapped` in `FileList.tsx` has two sentences. The root gets one that says
files sitting directly in it cannot be fetched, though a folder below can be
mapped on its own. Every other folder gets "this folder is not on this device —
map `<top>` with `coffret map` to fetch its files".

The second one is said over folders that hold no files — a subtree of nothing
but subfolders. The sentence is true and reads as though the person is being
told about something that is not on the screen. The root case was already told
apart for exactly this reason; widen the same treatment rather than inventing a
third voice, and let what the listing actually holds decide which sentence is
said.

### 3. A drop on an unmapped root refuses every file, including the ones a mapped folder below could take

`takesADrop = listing.mapped` is the whole of the decision. Dropping a nested
folder onto an unmapped Library root refuses the drop entire, even where a
top-level folder below the root **is** mapped and could have taken its share.

Decide what a drop means where the mapping is partial. Two shapes are open:
refuse the whole drop but say which part could have been taken, or take the
part that has somewhere to go and say what was left. The second is better for
the person and costs a decision about what a half-accepted drop reports; the
first is honest and cheap. Choose, write the reason in a comment, and make the
answer name the folders either way — a refusal that does not say which part was
refusable is the defect this item is about, and it survives both shapes.

### 4. `/api/list` answers the same empty 200 for a folder that is not there

`list` hands `library.list(folder)` whatever path it was given and serializes
what comes back. A folder the Library has never held produces an empty listing,
which is byte-for-byte what an empty folder produces.

The two are distinguishable, and the Index already holds the fact: a folder
exists in the Library because something is under it, so a real folder cannot be
empty. Say so. Decide where the difference belongs — a refusal with its own
`kind`, or a field on the listing — and remember the explorer is not the only
caller: a hand-typed hash reaches this route, and so does a stale link. What a
person sees for a mistyped path should not be an empty folder.

### 5. A refusal about mapping arrives saying a fetch did not finish

`coffret_device::Error::Fetch` carries `local_path_of`'s `UnmappedEntryPath`
and `EntryNotCurrent`, so a failure that is about **where this device puts
things** reaches a caller inside a chain whose outer sentence is "the fetch did
not finish". Once item 1 makes the click answer, this is the sentence behind
the answer, which is why it belongs here rather than in a later error-shape
change.

Give `error.rs` a variant naming what actually failed — `local_path.rs` is the
one raiser, so a `map_err` there and a matching arm in the server's
`From<Error>` is the whole of it. Do not widen the change beyond those two
files and their tests; the rest of `coffret_device::Error`'s shape is a
separate change already recorded.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] clicking a file row in an unmapped folder produces an answer, and a test
      pins what it says
- [x] the banner over a folder that holds no files does not tell a person how
      to fetch its files, and a test pins both wordings
- [x] a drop where the mapping is partial names the folders it refused or took,
      and a test pins it
- [x] `/api/list` tells a folder that is not there from one that is empty, and
      a test pins both
- [x] a mapping refusal no longer reaches a caller inside a chain that says the
      fetch did not finish, and a chain test pins the sentence

### Manual / on-hardware (verified by a human before merge)

- [ ] `make e2e-it` is green
- [ ] an unmapped folder was clicked into, a file row in it was clicked, and a
      nested folder was dropped onto a root whose mapping is partial
- [ ] a mistyped folder path was typed into the address bar

## Out of scope

- The shape of the server's error answers in general — the unknown route that
  escapes the one JSON form, the listing limit classified as Storage not
  answering, and `EpochActivated` reaching the page as a 502. Its own change
- The rest of `coffret_device::Error`'s shape, and `coffret-shell`'s `anyhow`
  in public signatures
- The vocabulary questions around `mapping`, `remote` and `present`. They are
  one docs pass and get their own change
- `Content-Disposition` on the download of an unsupported format
