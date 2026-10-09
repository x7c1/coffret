# Commit Protocol

Rule prefix: `CP`. How a batch becomes part of the Library: the Journal head
and its commit slot, the Keyring candidate a commit selects, how Master Key
epoch activation fences concurrent writers, and what an uploaded Container is
checked against before a batch names it.

Concept background: [Journal](../../concepts/journal/),
[Keyring](../../concepts/keyring/), [Master Key](../../concepts/master-key/).

## Rules

- **CP-1.** The Journal record is the commit point of a batch: before the
  record exists, the batch has not changed the current Container set; once it
  exists, its additions and removals are part of that set. *(Form: test)*
- **CP-2.** Each authenticated control head determines exactly one next
  commit slot, and exactly one successor may consume it: an ordinary Journal
  record, or the Index Snapshot that activates a new Master Key epoch. The
  head carries the slot in whatever form the Storage identifies objects by:
  the successor's object name where names identify objects, or a pre-minted
  identifier where the Storage mints identifiers. The conditional create of
  CP-3 targets exactly that slot. *(Form: test)*
  - What a head persists in `next_commit_slot` — and in CK-10's
    `snapshot_slot` — is the Storage's own opaque token and nothing else: the
    pre-minted identifier where the Storage mints identifiers, and nothing at
    all where it does not. The name is not persisted beside it; it is
    re-derived at the moment the slot is consumed, from the head's generation
    and the successor's role (CP-15, FM-12), so the two spellings cannot drift
    apart.
- **CP-3.** Both successor kinds use conditional create against the same
  slot, so of the operations that start from the same head exactly one
  succeeds. This is what lets epoch activation atomically fence writers that
  still hold the old epoch. *(Form: test)*
  - A refusal is a claim that the slot is taken, not proof of it: a Storage
    may refuse a conditional create because another one was in flight, and
    that one may then have failed, leaving the slot free. A refused writer
    therefore reads the slot back before concluding anything (CP-4, CP-5,
    CK-11), and a slot that holds nothing means no successor was committed —
    the writer starts the commit again rather than treating the refusal as a
    final loss.
- **CP-4.** A writer whose slot was consumed by another Journal record has
  not committed; it refreshes the head, rebases its batch onto it (EP-7), and
  retries. *(Form: test)*
- **CP-5.** A writer whose slot was consumed by an activation Index Snapshot
  stops until it is re-enrolled in the new epoch. *(Form: test)*
- **CP-6.** A successful activation Index Snapshot carries the new epoch's
  next commit slot and becomes the head for later Journal records.
  *(Form: test)*
- **CP-7.** A commit conflict never selects a winner by timestamps or
  silently applies last-write-wins. If both sides changed the same Entry
  Path, the conflict requires explicit resolution before retrying (rebase
  recheck: EP-7). *(Form: test)*
- **CP-8.** Before a Journal commit, the writer computes the post-commit
  Container set as `(current - removals) union additions`, then writes and
  read-back verifies a complete candidate Keyring replica set (KL-2) whose
  Container IDs exactly equal that set. *(Form: test)*
- **CP-9.** The previously committed Keyring remains authoritative until the
  Journal commit, so the candidate excludes removed Containers without making
  the pre-commit state unreadable. *(Form: test)*
- **CP-10.** A Journal record commits to the candidate Keyring's
  `master_key_epoch`, generation, replica count, and `set_digest`; the digest
  binds the canonical complete key table from Container IDs to Key Envelopes
  and key-lost markers (KL-7).
  Successfully creating the record commits the batch and selects that exact
  Keyring replica set in one state transition. *(Form: test)*
  - A candidate with any different commitment is not selected, even if it has
    the same generation.
- **CP-11.** Journal additions carry each new Container's ciphertext hash,
  its kind, and its entry table — the values the meta section records
  (FM-9), under the catalog's own field names (FM-15) — and never carry Key
  Envelopes: which
  Containers are current is the Journal's responsibility, and the committed
  Keyring is the only Storage representation of the keys needed to open
  them. Journal records never serve as envelope copies, before or after
  `prune`. *(Form: test)*
  - The kind and entry table in a record are a copy of the Container's
    authenticated meta section, which remains the authority on what the
    Container holds; the copy is what lets a device replaying the record
    (CK-9) rebuild its Index without opening the Container.
- **CP-12.** A Journal record has no Container Key or Key Envelope: it is
  encrypted and authenticated directly with a purpose-specific key derived
  from the Master Key (RV-3), so the record that commits a batch is readable
  independently of the Keyring replica set. *(Form: test)*
  - Its own ciphertext hash is therefore not part of its additions.
- **CP-13.** Every Journal record belongs to exactly one Master Key epoch.
  *(Form: test)*
- **CP-14.** A Container ID removed by a committed Journal record is never
  added again; restoring the same contents creates a new Container with a new
  ID. Removal from the current set is therefore monotonic, which is what
  makes trashing an untrashed removal idempotent (OC-6). *(Form: test)*
- **CP-15.** A slot is consumed only under the name its role gives it for the
  head it came from: `head-<generation + 1>` for a commit (CP-2),
  `idx-<generation>` for that head's ordinary Index Snapshot (CK-10). A
  writer that finds itself about to create under any other name refuses and
  writes nothing. Consuming one slot under two names is what would let two
  successors of one head both succeed on a Storage that keys objects by name,
  which is exactly the exclusion CP-3 rests on. *(Form: test)*
- **CP-16.** Immediately before consuming a slot, a writer re-reads the head
  object the slot came from and aborts if it is gone. A later epoch's
  rotation permanently deletes old-epoch control objects (MR-3), and on a
  Storage that keys objects by name that frees the key of a slot already
  consumed; without the re-read, a writer that woke long after its epoch
  ended could create a successor into a position the Library has moved past.
  *(Form: test)*
  - On a Storage that mints identifiers the consumed identifier stays refused
    — Google Drive answers a create under a purged pre-minted id with `400`
    at the upload's final request — so there the re-read does not prevent the
    create; it spares the writer from streaming a whole object before being
    told, and keeps the rule one rule for both kinds of Storage.
- **CP-17.** Before a batch names a Container it uploaded, the writer checks
  the object against the digest the provider reports in its answer to that
  write — not a digest a later listing reports, which on a Storage that mints
  identifiers could speak for another object of the same name. What is compared is the
  provider's own digest of the stored bytes against the same kind of digest the
  writer took of the spooled ciphertext while writing the spool. A mismatch
  means the object is not the bytes that were sent: the run stops with the transfer
  reported as corrupted, and the batch is not committed, so no Journal record
  names the object; what it left is settled like any uncommitted upload
  (OC-2). *(Form: test)*
  - A provider that always reports a digest for a write and answers one
    without it is refused as a malformed answer, so the run stops rather than
    going on unverified. Only a provider that has no digest to give leaves an
    upload unchecked here, and that is recorded rather than refused.
  - This check is transfer integrity against one provider and nothing more:
    the end-to-end guarantee remains the ciphertext hash a Journal addition
    carries (CP-11, FM-15), which a reader verifies after fetching every
    parcel of the object; a reader of only some parcels relies on chunk
    authentication and each Entry's hash instead (PK-22).
- **CP-18.** Every Container in a batch's removals must still be current in
  the state the batch commits onto. A writer checks this before anything is
  written, against the head its catch-up reached — on the first attempt, which
  may meet a head that moved since the batch was prepared, and on every rebase
  (CP-4) — beside the Entry Path recheck (EP-6, EP-7). A batch that removes a
  Container no longer current is refused as a conflict naming every such
  Container, and no record of it is created. Committing it anyway would let a
  later write silently undo an earlier committed removal: a replacement landing
  for a one-file Container another writer already replaced or removed, a Pack
  absorbing a one-file Container another writer removed, or a Pack rebuilt by
  read-modify-replace (PK-10) bringing back Entries another writer deleted with
  the whole Pack (CP-7). *(Form: test)*
  - `sync`, `freeze`, and `delete` end the run with the refusal, as with any
    other refused commit, and never offer the same batch again: the next run
    re-plans from the new state. What the refused batch uploaded is settled
    like any upload whose batch did not commit (OC-2, OC-3): a batch refused on
    its first attempt has recorded no commit attempt and is disposed of by the
    next `sync`, and one refused on a rebase after losing the slot has, and is
    retained.
