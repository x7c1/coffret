# Recovery

Rule prefix: `RV`. What a restore requires, how recovery bootstraps its keys,
when recovery degrades to salvage, and what Keyring loss means.

Concept background: [Library](../../concepts/library/),
[Storage](../../concepts/storage/), [Index](../../concepts/index/),
[Storage Object](../../concepts/storage-object/).

## Rules

- **RV-1.** A restore reconstructs the current Container set from a valid
  Index Snapshot checkpoint followed by every later Journal record, or from
  the complete unpruned Journal history. For any pruned history, the Snapshot
  that covers it is the required baseline until a newer valid checkpoint
  supersedes it. *(Form: test)*
  - Both kinds of Index Snapshot are checkpoint candidates: an ordinary one
    under an `idx-` name and an activation one under a `head-` name carry the
    same checkpoint (CK-1, CK-2, CK-3), so a scan for the newest valid
    checkpoint considers both (FM-12).
  - A restore recovers each Entry under its current path, renames included.
    A Container records only the path its Entry was captured under (FM-9);
    the current path is the Journal's and its checkpoint's (FM-15, FM-16), so
    the restore names each Entry as the checkpoint names it — or, from the
    complete history, as the record that added it does — with every later
    record's renames applied (RV-5).
  - A restore recovers each Entry's deposit time the same way: as the
    checkpoint carries it, or as the record that added the Entry's content
    stamped it, unchanged by every later record's renames (FM-15, FM-16). A
    restore is not a deposit and stamps nothing of its own, so a device
    restored from a Library holds the same deposit time for every Entry as
    the devices that committed it.
- **RV-2.** A restore additionally requires at least one committed valid
  Keyring replica matching the tuple the checkpoint records (KL-3, CK-3) and
  the current Containers themselves; a degraded replica set permits the
  restore, with repair gated by KL-11. The restore reconstructs the exact
  current Container set, but it can open only those mapped to Key Envelopes;
  a Container mapped to a key-lost marker remains current and is reported as
  unreadable (KL-7, RV-7). *(Form: test)*
- **RV-3.** Recovery bootstrap is acyclic: purpose-specific keys derived from
  the Master Key directly open the control objects — Index Snapshots, Journal
  records, Keyrings — and the Keyring's envelopes then open the Containers.
  *(Form: test)*
  - Control-object keys are domain-separated by purpose: a key derived for a
    Journal record is never used for a Keyring or an Index Snapshot.
- **RV-4.** If the required checkpoint or Journal history is incomplete,
  recovery becomes salvage: coffret may present contents from decryptable
  Containers but cannot distinguish current Containers from removed,
  replaced, or uncommitted candidates. Salvage performs no automatic cleanup,
  never authorizes deletion or mutation, and is not a restore. *(Form: test)*
  - Salvage presents each Entry under the captured path its Container's meta
    section records (FM-9). Renames are carried only by Journal records and
    Index Snapshots (FM-15, FM-16), so salvage cannot know an Entry's current
    name and never presents a name as current. A Pack rebuilt after a rename
    records the name current at the rebuild as its captured path (PK-10), so
    salvage of that Pack shows that name — still a captured path, which a
    later rename leaves behind in turn.
  - Salvage has no deposit time to present: the time is carried only by
    Journal records and Index Snapshots (FM-15, FM-16), and no meta section
    records one (FM-9). A salvaged file is never shown as deposited at its
    `captured_mtime` or at any other time, and content salvaged and then put
    back into a Library is deposited anew, at the time of that deposit.
- **RV-5.** An exact rebuild of the Index's cached Catalog follows RV-1 and
  opens no Container: the complete unpruned Journal, or a checkpoint and later
  records, carries every current Entry (CK-7, CP-11). Device-local mappings,
  materialization records, and pending provenance are preserved during a cache
  rebuild and cannot be recovered from Storage after loss of the whole Index.
  Without the required control state, opening decryptable Containers yields
  salvage candidates, not the current Catalog.
  *(Form: test)*
  - Replay applies each record's `renames` with the rest of it: the record's
    removals are taken out, then all its renames are applied at once — each
    Entry named by `(Container ID, entry number)` takes its new `path` — then
    its additions are added, and the result holds to EP-5 (FM-15). A record
    one of whose renames breaks an FM-15 state rule is refused whole, and the
    cached Catalog stays as it was. A record whose generation is at or below
    the head the cached Catalog already stands at is refused before any of it
    is applied, leaving the cached Catalog and device-local records as they
    were, and a replay steps over it: another replay into the same Index has
    applied it (CK-9, CK-13). A checkpoint or a record of a schema before
    deposit times is refused rather than rebuilt from, since nothing could
    supply the time its Entries lack (FM-15, FM-16). A record that names in
    `required_schema` a schema above the one the reader implements stops the
    rebuild at that record, with the records before it applied and none of
    it, and a newer build is what reads on (FM-9, FM-15).
- **RV-6.** Authenticating Storage Objects proves their integrity, not their
  freshness: Storage can replay a coherent earlier Library state by
  withholding newer objects, so recent additions may disappear and removed
  Containers may reappear. Detecting that rollback is not a coffret
  requirement; preventing it would need a trusted checkpoint outside Storage
  and may be added as a separate feature if the threat model changes.
  *(Form: prose — an
  accepted limit against an adversarial Storage counterparty; a
  non-requirement has no test form)*
- **RV-7.** Zero committed valid replicas of the required Keyring is
  **Keyring loss**, a condition distinct from a degraded set: with no source
  replica, repair (KL-13) does not apply. The current Containers of that
  epoch are unreadable even with the Master Key; coffret enumerates the
  affected Containers — and their Entry Paths, where recoverable from
  readable control state — and reports them. Control objects themselves
  remain readable, because they are encrypted directly under keys derived
  from the Master Key (RV-3). Once a rebuild (RV-8) commits, the affected
  Containers are carried with key-lost markers — present but unreadable, visible
  to the user — rather than dropped from the current set. *(Form: test)*
- **RV-8.** After Keyring loss, a device holding authenticated local key
  material — for example cached decrypted Container Keys — MAY rebuild a new
  complete Keyring generation: envelopes for the Containers its material
  covers, key-lost markers (KL-7) for the rest, committed through the normal
  candidate-to-commit path (CP-8, CP-9, CP-10). The rebuild implies no removals
  and authorizes no deletion of any keyless Container's ciphertext; any
  device that later holds more material may commit a further generation
  upgrading markers to envelopes — an ordinary envelope-set update.
  *(Form: prose — a deliberate permission mandates no behavior a test could
  require; an implementation that takes this path is governed by the
  ordinary commit rules, which are test-bound)*
