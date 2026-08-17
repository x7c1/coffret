# Master Key Rotation

Rule prefix: `MR`. How a new Master Key epoch is activated and when a
rotation counts as complete.

Concept background: [Master Key](../../concepts/master-key/),
[Recovery Code](../../concepts/recovery-code/),
[Keyring](../../concepts/keyring/).

## Rules

- **MR-1.** Rotation re-wraps every current Container Key and refreshes the
  control objects — a few MB — under the new Master Key; Containers remain
  byte-for-byte unchanged. *(Form: test)*
  - Key-lost markers (KL-7) have no envelope to re-wrap; they carry over
    into the new epoch unchanged.
- **MR-2.** An activation Index Snapshot carries a full Index and checkpoint
  like an ordinary Index Snapshot. It additionally consumes the current
  commit slot (CP-2, CP-3), activating the prepared epoch and atomically
  fencing writers still on the old epoch; the activation Snapshot becomes the
  new head (CP-6). *(Form: test)*
- **MR-3.** Rotation is complete only after every old-epoch Keyring, Journal
  record, and Index Snapshot reachable by coffret has been permanently
  deleted — deleted, not trashed, because old-epoch control objects are
  exactly what a leaked old Recovery Code could open. *(Form: test for the
  deletion of every reachable old-epoch control object; Form: prose for the
  boundary that a copy retained by an attacker or the Storage provider before
  deletion remains readable with the old Master Key — it involves
  external-world state rotation cannot reach)*
- **MR-4.** Rotation creates a new Recovery Code carrying the new epoch;
  devices holding the previous Master Key stop at the activation fence (CP-5)
  and must be enrolled again with that code. *(Form: test)*
