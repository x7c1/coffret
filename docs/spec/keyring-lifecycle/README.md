# Keyring Lifecycle

Rule prefix: `KL`. When a Keyring replica is valid, when a replica set is
complete, committed, or degraded, what each state permits, and how a
degraded set is repaired.

Concept background: [Keyring](../../concepts/keyring/),
[Key Envelope](../../concepts/key-envelope/).

## Rules

- **KL-1.** A replica is **valid** when it decrypts and authenticates
  successfully, its epoch, generation, replica position and count are internally
  consistent, and the `set_digest` its name carries (FM-12) is the digest of
  the canonical complete key table from Container IDs to Key Envelopes and
  key-lost markers (KL-7) in its payload (FM-17). *(Form: test)*
- **KL-2.** A replica set is **complete** when its valid replicas agree on
  one epoch, generation, replica count, and `set_digest`, and every replica
  position declared by that count is present exactly once. A candidate set can
  reach completeness before any commit selects it. *(Form: test)*
- **KL-3.** A replica set becomes **committed** only when a successful
  Journal commit or Master Key epoch activation selects its exact commitment
  tuple — `master_key_epoch`, generation, replica count, and `set_digest`
  (CP-10). A valid replica matching that tuple is a committed valid replica.
  *(Form: test)*
- **KL-4.** An ordinary Index Snapshot records a tuple that was already
  committed; after covered Journal records are pruned, the Snapshot preserves
  the evidence of that earlier selection. Placing a reference to a candidate
  set in a Snapshot leaves the candidate uncommitted. *(Form: test)*
- **KL-5.** A committed replica set with at least one valid replica but fewer
  than the count its commitment selected is **degraded**. Zero committed valid
  replicas is **Keyring loss** (RV-7), not degraded. An incomplete uncommitted
  set is a **partial candidate**, also distinct from degraded. *(Form: test)*
- **KL-6.** One committed valid replica contains the complete logical Keyring
  payload; the replica count provides redundancy against individual object
  loss and carries no quorum semantics. *(Form: test)*
- **KL-7.** At every successful commit or `prune` boundary, the committed
  Keyring maps every current Container and no non-current one. Each is
  mapped either to its Key Envelope or to an explicit **key-lost marker**
  recording that the committed control state has no reachable envelope for
  it. The marker makes no claim about authenticated local key material outside
  Storage, which may later restore an envelope through RV-8. *(Form: test)*
- **KL-8.** A generation's commitment selects the replica count required for
  that generation; a newly prepared generation uses the current replica
  policy, whose initial value is three. *(Form: test)*
- **KL-9.** Replication is effective only within one generation: envelopes
  introduced by a newer generation are protected only by that generation's
  replicas, never by older generations still on Storage, which `prune`
  disposes of once superseded (KL-18). *(Form: test)*
- **KL-10.** Every Keyring generation belongs to one `master_key_epoch`; the
  generation numbers the successive envelope sets across the Library's whole
  life and never restarts at a rotation, so a generation number is never
  reused and determines its epoch. *(Form: test; not yet implemented:
  generations across a Master Key rotation)*
- **KL-11.** Restore may use any one committed valid replica. If at least one
  but fewer than the committed count remain, restore proceeds with the
  degraded set, but the set must be repaired to complete before another
  write, `prune`, or Master Key rotation. *(Form: test; not yet implemented: the
  repair before a `prune` or a Master Key rotation)*
- **KL-12.** A valid replica set with no reachable committed Journal record
  or Index Snapshot is treated as a suspected orphan; its disposal follows the
  orphan-cleanup rules (OC-2, OC-3, OC-4). A superseded generation (KL-18)
  is not an orphan: it was committed, and `prune` disposes of it under KL-18
  instead. *(Form: test; not yet implemented: disposing of an orphan set OC-3
  proves)*
- **KL-13.** Repair is automatic: whichever device finds the committed
  replica set degraded (KL-5) rewrites the missing replicas from any
  committed valid replica, restoring the full committed count. Repair only
  re-materializes the committed generation — it never invents state and
  never deletes anything. Repair rewrites only the generation the device's
  head selects after it has caught up (CK-9); a device that finds replicas
  missing from an older generation catches up first and repairs nothing if
  that generation has been superseded (KL-18). *(Form: test)*
- **KL-14.** Replica objects are identified by generation and replica
  position — a generation belongs to exactly one epoch (KL-10) — and a replica
  at `(generation, set_digest, position)` has exactly one valid content: the
  canonical key table its digest binds (KL-1, KL-3). Two devices repairing the
  same replica therefore write the same key table under the same `set_digest`
  (FM-17); the stored objects still differ, each sealed with a random nonce of
  its own (FM-11), so what is identical is the content the digest covers and
  not the bytes on Storage. Repair is therefore an unconditional write and a
  duplicate is benign; there is no race whose loser needs reporting. On a
  Storage whose names are not unique a duplicate is a second object under the
  replica's name, beside a copy that did not read back valid or beside another
  repair's; a reader tries every object listed under the name, and the
  position is valid when any one of them reads back valid (KL-1), whatever
  order the listing gives them in. Repair confirms itself by reading the
  replica back, and what that read-back establishes is the replica's validity
  (KL-1) — not the exclusivity of the write, and not a byte comparison against
  what this device wrote.
  *(Form: test)*
  - Unlike a commit (CP-2), a repair has no shared reservation two devices
    could aim at, and needs none: exclusion matters where two writers would
    write *different* content, and here they cannot.
- **KL-15.** Replica loss and the repair performed are surfaced to the user
  as a health event; neither happens silently. *(Form: test)*
  - A run reports as loss only what a read established: a replica missing,
    or read and found invalid (KL-1). Of a position Storage would not hand
    over, it says loss there is not established, since the replica there may
    still be valid; a run that stepped over only such positions does not call
    the set degraded (KL-5).
  - A run that opens its Containers under a key table its process kept from
    an earlier read under the same Keyring commitment does not read the
    Keyring again, and carries the finding that earlier read made. A finding
    that the set was short is kept only until this process next catches up
    or runs a write (sync, freeze, delete); either one forgets it, so the
    next run reads the set again and reports only what that read finds.
  - A repair keeps the commitment it had (KL-13, KL-14), so the commitment
    alone cannot tell a repaired set from the one a short finding was about.
    A key table read from a whole set may be reused across a catch-up that
    leaves the commitment unchanged.
- **KL-16.** If repair cannot complete — write failures, quota, permissions
  — the completeness gate holds unchanged: writes, `prune`, and Master Key
  rotation remain refused (KL-11), while reads and restore remain allowed
  from the surviving committed valid replicas. The failure is reported and
  retried; the gate is never partially relaxed. *(Form: test)*
- **KL-17.** A current Container mapped to a key-lost marker by the committed
  Keyring remains current: nothing authorizes deleting its ciphertext, and it
  leaves the current set only through a genuine committed removal — the user
  deletes all of its Entries (PK-9), or `update` replaces it, for which a
  key-lost Container is always eligible while its local file survives (PK-11,
  PK-12), healing the loss. A deletion of only some of its Entries is refused
  for it, because keeping the others would need a read-modify-replace no
  device can perform (PK-10). *(Form: test)*
  - Healing the loss is no deposit by itself. A deposit is new content,
    decided by its hash (FM-15): a replacement re-encrypting local plaintext
    whose `hash` is the key-lost Entry's keeps that Entry's deposit time,
    whether `update` or `freeze` (PK-13) builds it, and only a replacement
    whose content differs is stamped with a new one.
- **KL-18.** A Keyring generation is **superseded** once a later generation
  of the same epoch is committed and complete (KL-2, KL-3). `prune` disposes
  of every replica of each superseded generation older than the generation
  its Snapshot records (CK-3, CK-5), after it has deleted the Journal records
  that Snapshot covers (CK-4, CK-6), and never of the generation the Snapshot
  records or the generation the current head selects. Disposal is a
  recoverable `trash`, the same removal `prune` applies to Journal records;
  only a Master Key rotation deletes Keyring replicas irreversibly (MR-3).
  *(Form: test; not yet implemented: disposal in a Library past its first
  epoch, which keeps every generation until rotation is implemented)*
  - Nothing left on Storage after `prune` reaches such a generation: every
    current Container is mapped by the Snapshot's generation or a later one
    (KL-7), the Journal records that selected the older generations are gone,
    and an older Index Snapshot that records one can no longer be replayed to
    the head, so it is not a starting point a recovery could use (CK-9, RV-1).
  - A replica of a superseded generation that is missing is not a degraded
    set (KL-5) and is never repaired (KL-13).
  - A device that read an older head a moment ago has already read its key
    table; a generation disposed of under it does not stop the fetch it is
    making, and `trash` keeps the rest recoverable.
  - Replica names carry a generation and not a commitment (KL-14), so every
    set listed under a superseded generation's number goes with it, a
    candidate that lost its commit slot at that number included, which
    KL-12 would otherwise leave to orphan cleanup: no current Container
    depends on it (KL-7), and `trash` keeps it recoverable.
  - A Journal record `prune` left in place still selects the generation it
    committed, so where one is left no generation is disposed of in that run;
    the next `prune` takes both.
