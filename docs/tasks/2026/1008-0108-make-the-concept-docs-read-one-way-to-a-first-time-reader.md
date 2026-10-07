---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make spec-citations spec-rule-ids && G="$(find docs/concepts -name README.md -exec cat {} + | tr -s " \n" "  ")" && for p in "Container remains locked" "one per Storage location" "re-wraps every Container Key" "plaintext catalog" "whose mapping covers" "folding admits 128 spellings" "one rule seen twice" "is a socket" "moving the file removes the old position"; do if printf "%s" "$G" | grep -qF "$p"; then echo "still present: $p"; exit 1; fi; done'
assignee: null
branch: task/1008-0108-make-the-concept-docs-read-one-way-to-a-first-time-reader
created_at: 2026-10-07T16:08:08Z
updated_at: 2026-10-07T16:27:08Z
---

# docs(concepts): make the concept docs read one way to a first-time reader

## Overview

A reader given only the English concept docs under `docs/concepts/` (no spec,
code, ADR or prior discussion) was asked to explain the concepts and walk a set
of scenarios through them. Several passages read two ways, contradict a
neighbouring passage, or lean on a term defined nowhere. This change fixes those
passages so the docs stand on their own. It changes wording only: no spec rule,
no code, and no behaviour changes. Where a fix needs a fact, take it from the
spec (`docs/spec/`) and the code as they stand; do not invent guarantees.

Concept docs state definitions and rules, not implementation steps. Keep each
fix inside the doc that owns the concept, prefer linking the owning doc over
restating it, and do not grow a paragraph by appending clauses — rewrite the
sentence so it says one thing.

**1. Passages that contradict each other.** Make each pair say one thing.

- `entry-path/README.md` says moving a file "removes the old position and adds
  the new one", while `container/entry/README.md` describes the planned rename
  that keeps the same bytes in the same Container. Say what moving a file does
  today and that rename, once implemented, keeps the Entry; link the Entry doc.
- Rotation scope: `README.md` says rotation "re-wraps the available envelopes",
  `container/container-key/README.md` says it "re-wraps every Container Key",
  `key-envelope/README.md` and `master-key/README.md` say "every". A key-lost
  Container has no envelope to re-wrap. State the same scope in all of them.
- `pack/README.md` justifies grouping by "rebuilding without an Index —
  scanning every object on Storage — still finishes", while
  `catalog/README.md` says listing Storage "cannot reconstruct the current
  Catalog". Say what such a scan is for and what it can and cannot rebuild, so
  the two agree.
- `recovery-code/README.md` says a key-lost Container "remains locked", but
  `locked` elsewhere describes whether a running Library holds its Master Key.
  Use a word that does not collide (the Container stays key-lost / unreadable).
- `library/README.md` opens with "A user may keep more than one — say one per
  Storage location", then says one Storage location can hold several
  Libraries. Replace the example with one that does not suggest a one-to-one
  pairing.

**2. Concept names used in another sense.** Where a concept's name appears in
lower case for something else, use another word or link the concept when it
is the concept that is meant.

- "catalog" for something other than the Catalog: `container/README.md`
  ("no external catalog is needed"), `mapping/README.md` ("a plaintext
  catalog", "both still catalog the whole Library"), `library/README.md`
  ("restore the same catalog", "the catalog answers the same way", "still
  catalogs the whole Library").
- "mapping" for the Keyring's Container-to-envelope table, which collides with
  the Mapping concept: `README.md`, `key-envelope/README.md`,
  `keyring/README.md`, `journal/README.md`, `storage-object/README.md`, and
  `entry-path/README.md` ("caches the mapping from Entry Path to Entry
  location"). Pick one word for the Keyring's table, use it everywhere, and
  define it once in the Keyring doc.
- Key Envelope vs the account-cache key envelope: `key-envelope/README.md`
  says "Only Containers are opened through envelopes" while `README.md`
  mentions an account-cache key envelope. Say plainly that the
  account-cache key envelope is a different thing and where it is defined.
- `container/entry/README.md` "Changing only an Index path would not implement
  rename": say what is meant without implying the Index owns paths.
- `keyring/README.md` defines "degraded" both in the state table and relative
  to a run; make the two uses distinguishable.
- `index-snapshot/README.md` says an activation Snapshot occupies a head
  position "rather than checkpointing one" and also "carries the same full
  checkpoint"; say which is which.

**3. Terms used as if defined.** At first use in each doc, link the doc that
defines the term, or define it in a clause if no doc does: explorer, desktop
app, commit slot, salvage, Keyring completeness gate, recorded tuple (or name
it as the Keyring commitment), `behind`, arm / displace / supersede, the server
key written at start-up, marker, read-modify-replace, eligible, device backup,
authenticated local key material, entry table, the register.

**4. Sentences a first-time reader could not follow.** Rewrite each so it can
be read without the discussion it came from:

- `library/README.md`: "and the catalog answers the same way: it is reported
  `behind` exactly when it says what stopped its last catch-up"; "A browser
  that asks is told which, so a page left open over what it decrypted gives
  that plaintext up rather than holding it until its next request is
  refused"; "The key is one running server's and is drawn again at every
  start"; "A listing names such a folder rather than drawing it, so that a new
  folder is never made over files already there"; "because what such an app
  has that the Library has not is a socket".
- `mapping/README.md`: the `reach` collocation ("what a listing's `mapped` says
  of the folder it lists…"); "so the explorer is told so before it asks for one
  rather than after a round trip to Storage".
- `entry-path/README.md`: "folding admits 128 spellings, so passing over all of
  them would charge it 128 times over…"; "so the three rules above and that
  account are one rule seen twice"; "declines each placement refused for the
  first reason beside what it placed and fails the whole request on the
  second".
- `journal/README.md`: "A concurrent content update must follow an intervening
  rename".
- `index-snapshot/README.md`: "The records a Snapshot applies become deletable
  only behind the Keyring completeness gate".
- `pack/README.md`: "Those particular Packs are local to that invocation".
- `master-key/README.md`: "Surviving local plaintext or authenticated Container
  Key material lies outside that restore guarantee."
- `passphrase/README.md`: "The Passphrase protects only the stored Master
  Key: …" — make clear what "only" restricts.

Out of scope, because separate changes will rewrite these passages with new
behaviour: what range reads and the fetch unit promise (`container/README.md`
"Fetched whole", the `pack/README.md` example "fetching one volume brings its
neighbors along" and the rule "the rest of the Pack is as unfetched
afterwards"), what happens to files on other devices after an Entry is
removed, and the path a Passphrase takes from the desktop app's window to the
server. Leave their meaning as it is.

Japanese copies of the concept docs are maintained outside this repository and
are not part of this change.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] No concept doc says a key-lost Container "remains locked"
- [x] The Library doc no longer offers "one per Storage location" as its example of keeping several Libraries
- [x] No concept doc says rotation "re-wraps every Container Key"
- [x] "plaintext catalog" and "whose mapping covers" no longer appear; the Keyring's table has one name of its own
- [x] The Entry Path doc no longer says moving a file "removes the old position" without reference to rename
- [x] The entry-path sentences "folding admits 128 spellings" and "one rule seen twice", and the library sentence ending "is a socket", are rewritten
- [x] Spec citations in the docs still resolve (`make spec-citations spec-rule-ids`)

### Before merge (verified outside the check command)
- [ ] Reading the diff confirms every passage listed in sections 1–4 of the Overview is addressed, and no definition or rule changed meaning
