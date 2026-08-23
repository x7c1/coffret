# Commit Protocol

Rule prefix: `CP`. How an upload batch becomes part of the Library: the
Journal head and its commit slot, the Keyring candidate a commit selects, and
how Master Key epoch activation fences concurrent writers.

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
    re-derived at spend time from the head's generation and the successor's
    role (CP-15, FM-12), so the two spellings cannot drift apart.
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
    settled loss.
- **CP-4.** A writer whose slot was consumed by another Journal record has
  not committed; it refreshes the head, reconciles, and retries. *(Form: test)*
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
  binds the canonical complete mapping from Container IDs to Key Envelopes
  and key-lost markers (KL-7).
  Successfully creating the record commits the batch and selects that exact
  Keyring replica set in one state transition. *(Form: test)*
  - A candidate with any different commitment is not selected, even if it has
    the same generation.
- **CP-11.** Journal additions carry each new Container's ciphertext hash,
  its kind, and its entry table — what the meta section records (FM-9), in
  the meta section's vocabulary — and never carry Key Envelopes: which
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
  makes removal completion idempotent (OC-6). *(Form: test)*
- **CP-15.** A slot is spent only under the name its role gives it for the
  head it came from: `head-<generation + 1>` for a commit (CP-2),
  `idx-<generation>` for that head's ordinary Index Snapshot (CK-10). A
  writer that finds itself about to create under any other name refuses and
  writes nothing. Spending one slot under two names is what would let two
  successors of one head both succeed on a Storage that keys objects by name,
  which is exactly the exclusion CP-3 rests on. *(Form: test)*
- **CP-16.** Immediately before spending a slot, a writer re-reads the head
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
