# Orphan Cleanup

Rule prefix: `OC`. When a Container that no reachable Journal record or
checkpoint mentions may be deleted, and what happens when orphanhood cannot
be proven.

Concept background: [Journal](../../concepts/journal/),
[Storage](../../concepts/storage/).

## Rules

- **OC-1.** A Container outside the reconstructed current set is necessary
  but not sufficient evidence of an uncommitted orphan: Storage may be
  withholding the newer Journal record that made it current, so reconstructing
  the current set authorizes restore, never garbage collection.
  *(Form: prose — quantifies over adversarial withholding by Storage, which
  no test can express; its concrete no-delete consequences are the Form: test
  rules OC-2 to OC-4)*
- **OC-2.** Automatic cleanup of a suspected orphan requires positive local
  provenance that identifies the creating batch, plus proof that the batch
  did not commit. *(Form: test)*
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
  removal completion are both idempotent (CP-14). *(Form: test)*
