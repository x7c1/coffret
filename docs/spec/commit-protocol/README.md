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
  record, or the Index Snapshot that activates a new Master Key epoch.
  *(Form: test)*
- **CP-3.** Both successor kinds use conditional create against the same
  slot, so of the operations that start from the same head exactly one
  succeeds. This is what lets epoch activation atomically fence writers that
  still hold the old epoch. *(Form: test)*
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
  binds the canonical complete mapping from Container IDs to Key Envelopes.
  Successfully creating the record commits the batch and selects that exact
  Keyring replica set in one state transition. *(Form: test)*
  - A candidate with any different commitment is not selected, even if it has
    the same generation.
- **CP-11.** Journal additions carry each new Container's ciphertext hash and
  never carry Key Envelopes: membership is the Journal's responsibility, and
  the committed Keyring is the only Storage representation of the keys needed
  to open the current Container set. Journal records never serve as envelope
  copies, before or after `prune`. *(Form: test)*
- **CP-12.** A Journal record has no Container Key or Key Envelope: it is
  encrypted and authenticated directly with a purpose-specific key derived
  from the Master Key (RV-3), so the record that commits a batch is readable
  independently of the Keyring replica set. *(Form: test)*
  - Its own ciphertext hash is therefore not part of its additions.
- **CP-13.** Every Journal record belongs to exactly one Master Key epoch.
  *(Form: test)*
- **CP-14.** A Container ID removed by a committed Journal record is never
  added again; restoring the same contents creates a new Container with a new
  ID. Membership removal is therefore monotonic, which is what makes removal
  completion idempotent (OC-6). *(Form: test)*
