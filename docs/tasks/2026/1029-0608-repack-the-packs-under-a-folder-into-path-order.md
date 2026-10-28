---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1029-0608-repack-the-packs-under-a-folder-into-path-order
created_at: 2026-10-28T21:08:10Z
updated_at: 2026-10-28T21:55:39Z
---

# feat: repack the Packs under a folder into path order

## Overview

`freeze` groups Entries into Packs by Entry Path only within one invocation
(PK-8), and a rename leaves a renamed Entry in the Pack it was frozen into.
Over time a folder's books end up spread across Packs that also hold other
folders' books, and their path ranges interleave: reading ahead into the next
volume costs more requests, and a deletion's read-modify-replace (PK-10) reads
more unrelated Entries. PK-8 names the remedy — "the separate repack or
compaction operation" — and PK-10 and PK-15 already say how such an operation
carries Entries and what kind it creates, but the operation does not exist.

Add it, as a CLI command only. The explorer gets no control for it.

### The operation

`repack` runs for one Library over one folder (an Entry Path prefix, the way
`freeze --under` names one):

1. **Select.** Every current Pack that holds at least one Entry under the
   folder. One-file Containers are not inputs: absorbing them is `freeze`'s
   job (PK-1, PK-7). All of a selected Pack's Entries take part, including any
   outside the folder — a Pack is replaced whole, so its other Entries have to
   go somewhere, and they go into the new Packs in path order like the rest.
2. **Cut.** Sort the selected Entries by their current Entry Path and cut them
   with the same segmentation `freeze` uses (PK-3, PK-4, PK-6), with the same
   size target and the same `--target` override.
3. **Keep what is already in place.** A cut segment that holds exactly the
   Entries of one selected Pack, in the same order, with every captured path
   equal to its current path, is that Pack: it is kept, neither rewritten nor
   removed. So a folder that is already in order commits nothing, and running
   `repack` twice in a row commits nothing the second time.
4. **Rewrite the rest** by read-modify-replace's carrying rules (PK-10): read
   and verify every Entry of each Pack being replaced, carry it under its
   current Entry Path (which becomes its captured path), keep its
   `captured_mtime`, `captured_btime`, hash and deposit time, and encrypt each
   new Pack under a new Container Key. Every new Container is a Pack (PK-15).
   Reuse the existing read-modify-replace and freeze building blocks rather
   than writing a third Pack writer, and spool the way they do — never hold a
   whole Pack in memory.
5. **Commit once.** One Journal batch: the replaced Packs in removals, the new
   Packs in additions (CP-1, CP-14), through the ordinary commit protocol, so
   a concurrent rename or deletion on another device is handled the way it is
   for a deletion's rewrite today.
   - If any Pack being replaced cannot be read and verified — including a Pack
     mapped to a key-lost marker (KL-7, KL-17) — commit nothing at all and
     report the Pack. A partial repack would leave the order half-made while
     looking done; running again after the cause is fixed does the whole job.
   - The replaced Packs go to the provider's trash as removed Containers
     always do (OC-6).
6. **Report**: how many Packs were selected, kept, removed and added, and the
   bytes uploaded.

The spec gets a rule of its own for the operation under the `PK` prefix (the
next free number), stating steps 1 to 5 as rules with `(Form: test)`, and
PK-8's last sentence links to it. Tests against the in-memory store pin each
of them: selection (Packs with any Entry under the folder; one-file
Containers untouched), path order across what used to be two interleaved
Packs, a renamed Entry moving into its new folder's Pack, the kept-Pack rule
and the no-op second run, carried times and hashes, the all-or-nothing refusal
on an unreadable or key-lost Pack, and the single batch's removals and
additions.

### The command

`coffret repack --under <folder>` (taking the Library the way the other
Library commands do, `LibraryArgs`; `--target` as `freeze` takes it;
`--passphrase-stdin`; `--json` like the others). `--under` is required: a
repack of the whole Library rewrites everything not already in place, and
that should be asked for by naming the folder, the top level included
(document how to name it). It runs only when asked.

### Out of scope

- Any automatic repack (after a rename, a deletion, or a freeze).
- The explorer.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The spec has a `PK` rule for repack that PK-8 links to, bound to tests
      by the spec-rule checks in `make check`
- [x] Usecase tests pin selection, path-order cutting across interleaved
      Packs, a renamed Entry regrouped with its new folder, the kept-Pack rule
      and the no-op second run, carried times and hashes, the all-or-nothing
      refusal, and the one batch's removals and additions
- [x] `coffret repack --help` lists the command, and a CLI test covers its
      text and `--json` answers and the refusal of a missing `--under`
- [x] `make check` passes

### Before merge (verified outside the check command)

- [x] On the local Library (`make local`, MinIO): rename a volume of the
      twelve-volume series out to another folder and back, or rename a book
      into the series' folder; `coffret repack --under` that folder reports
      Packs removed and added, the explorer then lists the folder's books and
      the reader opens pages from them, and a second `repack` right away
      commits nothing
