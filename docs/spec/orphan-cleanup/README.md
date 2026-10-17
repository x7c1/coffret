# Orphan Cleanup

Rule prefix: `OC`. When a Container that no reachable Journal record or
checkpoint mentions may be deleted, and what happens when orphanhood cannot
be proven. The provenance a cleanup rests on can also prove the opposite —
that the batch did commit — and what that obliges instead is here too, along
with the idempotence of removing the local files and rows a device wrote for
itself.

Concept background: [Journal](../../concepts/journal/),
[Storage](../../concepts/storage/).

## Rules

- **OC-1.** A Container outside the reconstructed current set is necessary
  but not sufficient evidence of an uncommitted orphan: Storage may be
  withholding the newer Journal record that made it current, so reconstructing
  the current set authorizes restore, never garbage collection.
  *(Form: prose — quantifies over adversarial withholding by Storage, which
  no test can express; its concrete no-delete consequences are the Form: test
  rules OC-2, OC-3, OC-4)*
- **OC-2.** Automatic cleanup of a suspected orphan requires positive local
  provenance that identifies the creating batch, plus proof that the batch
  did not commit. *(Form: test)*
  - The provenance is recorded before the ciphertext it accounts for exists: a
    device writes the row naming a Container it is about to spool before the
    spool file is created, so every local ciphertext it produces is named by a
    row from the moment it can exist, and an interruption at any point leaves
    nothing whose creating batch cleanup cannot identify. Whether what a row
    names may then be removed is what this rule's second half asks, and an
    ambiguous slot leaves that unanswered (OC-3, OC-4).
  - Producers and settlement exclusively own this device's pending work for
    their whole execution. A competing run is refused before reading or
    reclaiming those rows, because another live producer may still use them.
  - Before a Journal create can reach Storage, its additions' pending rows
    durably record that a commit may have been attempted, and the batch
    durably records the commit slot that create is about to consume (CP-2), in
    the same all-or-nothing write: there is never a marked row without its
    slot, or a slot without its marked rows. A batch that rebases (CP-4)
    records each slot it attempts, so it can hold several. Uncertain attempts
    retain their ciphertext and provenance until OC-3 proves them; a legacy row
    without this evidence, or one whose batch recorded no slot, is treated as
    uncertain. Failed trash retains its proof for a later retry.
- **OC-3.** Two proofs qualify: the batch was abandoned before any commit
  attempt, or an authenticated different writer's record occupies the
  attempted commit slot. An empty, unavailable, or ambiguous slot is not
  proof. *(Form: test)*
  - An authenticated different writer's record is an authenticated control
    object at the attempted slot that does not name the row's Container among
    its additions. A Container ID is never reused (CP-14), so such a record
    cannot be this batch's, whichever device wrote it, a later run of this
    device included. An activation Index Snapshot at the slot is a different
    successor and qualifies as well. A record at the slot that does name the
    Container is proof the batch committed there, and the row is completed
    (OC-7) rather than reclaimed. That holds even where a later record has
    since removed the Container, and then none of its Entries is recorded as
    materialized, since none of them is current (EP-10).
  - The proof needs every slot the batch recorded (OC-2), since any one of them
    may be where its create landed. A slot that is empty, cannot be read or
    authenticated, or holds a record that is gone — pruned, or deleted with its
    epoch — is not proof, and the row is retained. A row whose batch recorded
    no slot is retained.
- **OC-4.** A suspected orphan without that provenance is retained and may be
  reported for manual review; recovery never deletes it merely because no
  reachable Journal record or checkpoint mentions it. *(Form: test)*
- **OC-5.** If an available authenticated Key Envelope makes a retained
  suspected orphan decryptable, coffret may present its authenticated
  contents in isolation; after warning that a withheld Journal record could
  still make it current, coffret may let the user explicitly trash it.
  *(Form: test)*
- **OC-6.** An **untrashed removal** is a Container a committed Journal record
  took out of the current set whose object no device has yet trashed. Any
  later run may trash it, and proven orphan cleanup and that trashing are both
  idempotent (CP-14). *(Form: test)*
  - An untrashed removal is not a suspected orphan (OC-1): its removal is proven
    by the record rather than inferred from absence, so the no-delete posture of
    OC-1 and OC-4 does not apply to it.
- **OC-7.** Local provenance whose Container is current in a caught-up Index is
  proof that its batch *did* commit, since nothing after the record can
  un-commit it (CP-1). Cleanup's action there is not reclamation but completion
  of the creating device's interrupted bookkeeping: the record of which Entries
  that device materialized in producing the Container is completed (EP-10), and
  the local ciphertext and the provenance itself are disposed of. The
  Container's object is left where it is, being the Library's. *(Form: test)*
  - Completion requires the provenance to say its spool is whole; a row still
    `Spooling` is never completed, whatever the current set says. Such a row
    names a spool this device announced and never finished, so nothing uploaded
    it and no record can name it, and it is disposed of as this device's own
    reclaimable leftovers (OC-2).
- **OC-8.** Removing what this device wrote for its own purposes — a spool
  file, a scratch a local writer never published (EP-11), the staging directory
  an interrupted attempt at putting a Library on this device left, or the
  provenance row (OC-2) in the device's own catalog that announced the spool —
  is idempotent. What is already gone is a successful removal, an interrupted
  clean-up is simply run again, and no removal has to check what is there
  first, because absence is the outcome being sought. *(Form: test)*
  - This is the posture CP-14 gives the Storage side, stated for the local one
    rather than borrowed from it. OC-6 says trashing an untrashed removal is
    idempotent, which is a claim about the Library's objects on Storage; a
    device's own leftovers are not the Library's, so their removals need a rule
    of their own and do not widen OC-6.
