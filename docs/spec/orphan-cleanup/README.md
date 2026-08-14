# Orphan Cleanup

Rule prefix: `OC`. When a Container that no reachable Journal record or
checkpoint mentions may be deleted, and what happens to candidates that
cannot be proven orphaned.

Concept background: [Journal](../../concepts/journal/),
[Storage](../../concepts/storage/).

## Rules

- **OC-1.** A Container outside the reconstructed current set is necessary
  but not sufficient evidence of an uncommitted orphan: Storage may be
  withholding the newer Journal record that made it current, so reconstructing
  the current set authorizes restore, never garbage collection.
  *(prose-only: quantifies over adversarial withholding by Storage, which no
  finite execution can enumerate; its concrete no-delete consequences are
  test-bound as OC-2 to OC-4)*
- **OC-2.** Automatic cleanup of a candidate requires positive local
  provenance that identifies the creating batch, plus proof that the batch
  did not commit. *(→ tests)*
- **OC-3.** Two proofs qualify: the batch was abandoned before any commit
  attempt, or an authenticated different writer's record occupies the
  attempted commit slot. An empty, unavailable, or ambiguous slot is not
  proof. *(→ tests)*
- **OC-4.** A candidate without that provenance is retained and may be
  reported for manual review; recovery never deletes it merely because no
  reachable Journal record or checkpoint mentions it. *(→ tests)*
- **OC-5.** If an available authenticated Key Envelope makes a retained
  candidate decryptable, coffret may present its authenticated contents in
  isolation; after warning that a withheld Journal record could still make
  the candidate current, coffret may let the user explicitly move it to
  trash. *(→ tests)*
- **OC-6.** Removals recorded by a committed Journal record but not yet
  physically deleted may be completed on recovery; proven orphan cleanup and
  removal completion are both idempotent (CP-14). *(→ tests)*
