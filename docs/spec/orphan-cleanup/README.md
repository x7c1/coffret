# Orphan Cleanup

Rule prefix: `OC`. When a Container that no reachable Journal record or
checkpoint mentions may be deleted, and what happens when orphanhood cannot
be proven. The provenance a cleanup rests on can also prove the opposite —
that the batch did commit — and what that obliges instead is here too.

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
    nothing cleanup cannot reach.
- **OC-3.** Two proofs qualify: the batch was abandoned before any commit
  attempt, or an authenticated different writer's record occupies the
  attempted commit slot. An empty, unavailable, or ambiguous slot is not
  proof. *(Form: test)*
- **OC-4.** A suspected orphan without that provenance is retained and may be
  reported for manual review; recovery never deletes it merely because no
  reachable Journal record or checkpoint mentions it. *(Form: test)*
- **OC-5.** If an available authenticated Key Envelope makes a retained
  suspected orphan decryptable, coffret may present its authenticated
  contents in isolation; after warning that a withheld Journal record could
  still make it current, coffret may let the user explicitly move it to
  trash. *(Form: test)*
- **OC-6.** Removals recorded by a committed Journal record but not yet
  physically deleted may be completed on recovery; proven orphan cleanup and
  removal completion are both idempotent (CP-14). Such a Container is an
  **untrashed removal**: a Container the committed record took out of the current
  set whose object no device has yet moved to the provider's trash.
  *(Form: test)*
  - An untrashed removal is not a suspected orphan (OC-1): its removal is proven
    by the record rather than inferred from absence, so the no-delete posture of
    OC-1 and OC-4 does not apply to it and any later run may complete the
    trashing — which is why completion is idempotent (CP-14).
- **OC-7.** Local provenance whose Container is current in a caught-up Index is
  proof that its batch *did* commit, since nothing after the record can
  un-commit it (CP-1). Cleanup's action there is not reclamation but completion
  of the creating device's interrupted bookkeeping: the record of which Entries
  that device materialized in producing the Container is completed (EP-10), and
  the local ciphertext and the provenance itself are disposed of. The
  Container's object is left where it is, being the Library's. *(Form: test)*
