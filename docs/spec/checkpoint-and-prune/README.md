# Checkpoint and Prune

Rule prefix: `CK`. What an Index Snapshot checkpoint records, which Journal
records become eligible for `prune`, the gate that must pass before they are
deleted, what a Snapshot carries beyond the checkpoint, when and where one is
uploaded, how a device brings a stale Index up to the head, what it holds
while it does, and how more than one process on a device shares one Index.

Concept background: [Index Snapshot](../../concepts/index-snapshot/),
[Journal](../../concepts/journal/), [Index](../../concepts/index/).

## Rules

- **CK-1.** An Index Snapshot records both the control-head generation it
  represents and the last Journal generation it applies; recovery starts from
  the head generation and replays the later Journal successors. *(Form: test)*
  - The last applied Journal generation is never past the head generation. The
    two coincide after an ordinary commit, where the head is the Journal record
    applied to reach it, and diverge only downwards at an epoch activation,
    whose Snapshot takes a head position without being a Journal record (CP-6,
    FM-13). A checkpoint claiming a Journal generation past its head names
    records applied to reach a state the head does not cover, so a reader
    refuses the pair rather than starting from it — in a Snapshot's payload
    (FM-16) and in an Index one was restored into alike (RV-5).
- **CK-2.** An ordinary Index Snapshot preserves the next commit slot from
  the Journal record it reflects; once that record is pruned, the Snapshot
  remains the source of the slot. *(Form: test)*
- **CK-3.** An Index Snapshot belongs to one Master Key epoch and records the
  exact committed Keyring tuple it depends on: `master_key_epoch`,
  generation, replica count, and `set_digest` (KL-3). *(Form: test)*
- **CK-4.** Journal records at or before the Snapshot's last applied Journal
  generation become eligible for `prune`. *(Form: test)*
- **CK-5.** `prune` may run only when the Snapshot preserves the exact
  committed Keyring tuple and that Keyring replica set is complete (KL-2) —
  otherwise deleting the records could destroy the only evidence or envelopes
  a recovery still needs. *(Form: test)*
- **CK-6.** `prune` deletes only eligible Journal records; it never deletes
  Containers or the Entries inside them. Its purpose is to bound
  retained Journal history and recovery replay. *(Form: test)*
  - `prune` is the formal operation name in documentation and code.
- **CK-7.** An Index Snapshot carries the Index of the whole Library — every
  current Entry and its Container, including Entries under subtrees the
  uploading device does not map (EP-9) — and carries no device state: no
  local root mappings, local paths, which Entries the device has materialized
  (EP-10), spool locations, or upload progress. Two
  devices that map different parts of one Library restore identical Indexes
  from the same Snapshot. *(Form: test)*
- **CK-8.** An Index Snapshot is written at three moments, by the device
  performing the operation: when the Journal committed since the newest
  checkpoint has grown past the checkpoint policy's threshold — judged by the
  committing device after its commit, which has just replayed that stretch —
  before `prune` (CK-4, CK-5), and at Master Key epoch activation (MR-2). It
  is not written after every commit: a commit pays for its own batch, never
  for the whole Library's Index. The threshold is a checkpoint-policy
  parameter, not a format constant; it weighs the replay a stale device
  would otherwise perform against the Snapshot upload that replaces it, and
  it is a trigger, not a bound — the record that crosses it, a Snapshot
  upload that fails, or a single oversized record can each leave more than a
  threshold's worth of Journal to replay until the next Snapshot lands. A
  failed Snapshot upload leaves the commit valid — the records it would have
  covered remain replayable — and the next qualifying moment writes one.
  *(Form: test for the moments and the failure behaviour; the threshold's
  value is a design decision recorded outside this register)*
  - A device that has only caught up (CK-9) and finds the Journal since the
    newest checkpoint past the threshold MAY write the current head's
    Snapshot into that head's `snapshot_slot` (CK-10); it is not obliged to.
    Whoever writes it, the result is one checkpoint (CK-11).
- **CK-9.** A device brings a stale Index up to the head from the newer of
  two starting points: its own Index, or the newest valid checkpoint — an
  ordinary Index Snapshot under an `idx-` name, or an activation Index
  Snapshot under a `head-` name, which are equally checkpoint candidates
  (FM-12). When the checkpoint is newer it adopts that Snapshot's
  Library-wide content and keeps its own device state (CK-7); when its own
  Index is newer, as it usually is between Snapshots, it keeps that. Either
  way it then replays only the Journal records committed after its starting
  point. Each record carries what the Containers it added hold (CP-11), so
  replay reads records and opens no Container, and the checkpoint policy
  (CK-8) keeps the stretch to replay near its threshold however long the
  device was away or however much other devices added. *(Form: test)*
  - A candidate that does not open is not valid, and neither is one whose
    declared length is past the ceiling its kind may be (FM-11): both are
    stepped over, and the walk goes on to the next older candidate rather than
    reporting the one it could not take. A length no writer produces is
    evidence about that one object, as a tag that fails to verify is, and a
    walk that stopped at it would let anybody with write access to Storage
    keep the device from ever catching up with a single object.
    A length this build cannot address is not stepped over: that is this
    device's own limit rather than anything wrong with the object, and it is
    reported. *(Form: test)*
  - Adopting a Snapshot another device wrote is safe for the same reason
    restoring from one is: it is authenticated under a purpose key derived
    from the Master Key (RV-3), and its checkpoint names the committed
    Keyring tuple it depends on (CK-3).
  - An Index records which checkpoint it adopted — the name of the Snapshot
    object its Library-wide content came from (FM-12) — so that a later
    catch-up reads its own starting point off the Index instead of going back
    to Storage for it. An Index that has only ever replayed records has
    adopted none, and a replay leaves the recorded checkpoint where it was: it
    is where this Index started, not where it now stands (CK-1).
  - Each device-side Index operation that writes — adopting a Snapshot,
    replaying a record, applying this device's own committed batch — is
    all-or-nothing. An operation interrupted part-way leaves the Index exactly
    as it was rather than half-applied, so a catch-up that failed is simply run
    again.
- **CK-10.** Each Journal record carries a `snapshot_slot`, reserved by its
  writer before the commit in the same form as a commit slot (CP-2): the one
  place where the ordinary Index Snapshot representing that head is created,
  by conditional create against it (CP-3). The Snapshot carries that head's
  generation and is named `idx-<generation>` for it (CP-15, FM-12, FM-13). An
  activation Snapshot is already the full checkpoint of the head it is, so no
  ordinary Snapshot is written for it. *(Form: test)*
  - The reason is that the second object would be a multi-megabyte duplicate
    of a checkpoint the Library already holds, not that its name would
    collide: an activation Snapshot is named for its place in the head chain
    and an ordinary one for the head it checkpoints, so the two names never
    meet (FM-12).
- **CK-11.** Losing that conditional create is not a failure. The loser
  reads the slot back: a valid Index Snapshot there that represents the same
  head (CK-1, CK-3) means the checkpoint exists and the loser's own upload
  is done — two Snapshots of one head would be the same checkpoint. Anything
  else at the slot is reported as Storage corruption and is neither
  overwritten nor written under another name, because a second name for one
  head would leave readers two checkpoints to choose between. *(Form: test)*
  - A slot holding nothing is not "anything else": the refusal settled
    nothing (CP-3), so the upload is attempted again rather than reported.
- **CK-12.** A device catching up holds at most 256 decoded control objects,
  and at most 64 MiB of their payloads, out of the walk down the checkpoint
  candidates it makes to find its starting point (CK-9). The walk keeps the
  Journal records it passes because the replay that follows comes back up over
  exactly those generations, so holding one saves fetching it twice; a record
  that would not fit under either budget is passed over, the walk carries on,
  and the replay reads that one from Storage again. The two numbers are this
  device's memory budget, not format constants; they weigh the second fetch a
  passed-over record costs against the memory holding it would take, and they
  are what this build holds at most, not a figure another implementation must
  match — one that holds fewer, or none, and reads every record twice is
  equally correct, because what the rule obliges is that the outcome not vary
  with how much is held. *(Form: test)*
  - Nothing else about the catch-up changes: the same starting point is
    adopted, and the same records are replayed in the same order to the same
    head. A single record too large to be held on its own is one of the ones
    read twice.
  - What the budgets bound is what one device holds in memory at once. They
    are not a bound on how far the walk goes, on how many records a replay
    applies, or on how large one record may be — FM-11 bounds that — and the
    stretch they are walked over has no bound of its own: CK-8's threshold is
    a trigger, so a Snapshot upload that never lands leaves that stretch
    growing until the next one does.
- **CK-13.** One Library's Index may be open in more than one process on a
  device at once — a server answering a browser while the same person runs a
  command at a terminal — and each stays usable while the other reads or
  writes. A read never waits on another process's write: the Index is kept in
  a form under which readers and one writer coexist, which for the prototype's
  catalog file is SQLite's write-ahead log. A write that meets another
  process's write waits for it, for a bounded time of the order of seconds,
  rather than failing at once and reporting the Index unusable, and each
  operation stays all-or-nothing (CK-9) whichever process makes it. The wait
  is this build's parameter, not a format constant: it weighs a listing
  somebody is looking at against one commit of one flow holding the write,
  which is what normally stands on the other side of it. *(Form: test)*
  - The rule lives here rather than under a prefix of its own because this
    mechanism already holds what a device's Index operations owe when they
    write (CK-9), and sharing the Index between processes is one more such
    obligation rather than a mechanism of its own.
