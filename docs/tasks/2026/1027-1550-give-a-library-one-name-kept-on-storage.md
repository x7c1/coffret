---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, spec-alignment, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1027-1550-give-a-library-one-name-kept-on-storage
created_at: 2026-10-27T06:50:48Z
updated_at: 2026-10-27T08:58:19Z
---

# feat: give a Library one name, kept on Storage, in place of the device-local name

## Overview

Today a Library has a **device-local Library name** (Library concept,
`docs/concepts/library/README.md`): each device picks one at `init` / `join`,
it names the directory `libraries/<name>/`, it is what `--library` takes, and
it is never written to Storage. A person knows a Library by one name across
devices, so replace the device-local name with a **Library name** that belongs
to the Library itself: chosen once at `init`, kept on Storage under the Master
Key, and read by every device that joins.

### The Library name

1. **Kept in the catalog.** The Library name is a field of the Journal record
   (FM-15) and of the Index Snapshot (FM-16), so it survives pruning the
   records before a checkpoint. A record may carry the name; the Library's
   name is the one carried by the latest record that carries it. An attribute
   a person may later want to change lives in the catalog, not in a
   Container's meta. The field is forward-open (a
   reader that ignores it loses only the name), so it does not raise
   `required_schema`; raise `schema` as FM-15 says adding a field does, and
   keep accepting records of schema 4 that lack it.
2. **Set at `init`.** `init --name <name>` writes the name in the generation-0
   record. The name's shape rules are today's device-local ones
   (`name_defect.rs`) unless a rule only existed because the name named a
   directory; say in the PR which rules were kept and why.
3. **Not changed in this task.** No command renames a Library yet (a later version
   may add one), but the format already lets a later record carry a new name.
4. **Still kept out of diagnostics.** EL-1 forbids the device-local Library
   name in a diagnostic event; it now forbids the Library name. The Library ID
   stays allowed (EL-5).

### On the device

5. **The directory is `libraries/<library id>/`.** Nothing a person chose names
   a directory any more. Keep the `.partial` staging (a directory under its
   real name is always a whole Library).
6. **A copy of the name for the list.** Before unlock nothing can reach Storage
   (the grant is behind the Master Key), so each unlock, `init` and `join`
   writes the Library's current name to the Library's `settings.json`. The
   Passphrase window's list (`coffret-desktop-core` `libraries()`) shows these
   names, and anything that shows the Library name today (the explorer's
   bottom line, for example) shows this one.
7. **`join` takes no name.** Drop `--name` from `join`; after the Recovery Code
   opens the Master Key, read the name from the catalog the join replays.

### Naming a Library on the command line

8. **`--library <name>` and `--library-id <id>`, never one flag for both.** A
   name may be any string, including one that spells another Library's ID, so
   one flag taking either could not tell them apart. Every command that takes
   `--library` today (CLI subcommands, `coffret-server`,
   `coffret-explorer-dev`) takes both, mutually exclusive, one required.
   `--library` matches the names in the devices' copies (item 6); where two or
   more Libraries on the device carry that name, refuse, listing their IDs and
   asking for `--library-id`.
9. Keep `scripts/dev.sh`, `scripts/e2e-electron.sh` and the e2e journeys
   working; they can go on naming Libraries by name.

### Libraries made before this change

10. **Moved when first opened by the new build, not recreated.** On unlock (and
    on any command that opens the Library), where the catalog carries no name:
    commit a record carrying the current directory's name as the Library name
    (a record with no additions or removals is allowed, FM-15), then rename the
    directory to `libraries/<library id>/` while no server holds it (the
    `server.lock`); where it is held, leave it for the next time. A directory
    found at either place is the same Library, so a command looking a Library
    up by name or ID finds it before and after the move. Two devices migrating
    the same Library at once may write two names; the later record wins, which
    is acceptable.
11. The production Library and the development `books` Library are migrated by
    their owner just by opening them with the new build. No Pack, Container or
    Keyring object is rewritten.

### Docs

- Library concept: replace the device-local Library name with the Library name
  (what it is, where it is kept, that it is shared by every device), and say
  the directory is named by the Library ID.
- Storage concept (`docs/concepts/storage/README.md`): it compares the
  device-local account name with the device-local Library name; fix that line.
- Spec: FM-15 / FM-16 (the field), EL-1 (the name), and any rule that says the
  Library name is device-local or never written to Storage. Name new or changed
  rules from their tests as the spec does.
- `coffret-device` crate docs (`lib.rs` describes `libraries/<name>/`), CLI
  help text, and `docs/` guides that show `init` / `join` / `--library`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `init --name N` writes N into the generation-0 record; a device that
      `join`s without a name finds N in its copy and its list
- [x] The Index Snapshot carries the name, and a device that catches up from a
      Snapshot alone reads it
- [x] A Library's directory is `libraries/<library id>/`
- [x] `--library N` and `--library-id I` both open the Library; giving both, or
      neither, is refused; two Libraries named N make `--library N` refuse with
      both IDs listed
- [x] A Library whose catalog has no name, in a directory named `old`, is given
      the name `old` and moved to `libraries/<library id>/` on open; while a
      server holds it, it is named but not moved, and is moved on a later open
- [x] No diagnostic event carries the Library name (an EL-1 test with a
      sentinel name)
- [x] The Passphrase window's list shows the names from the copies
- [x] `make check` passes

### Before merge (verified outside the check command)

- [x] On the agent's Library (`~/.local/state/coffret-agent`, MinIO
      `coffret-minio-agent`): opening `agent` with the new build names it
      `agent` and moves its directory to `libraries/<library id>/`; the server
      and explorer started with `--library agent` work, and the listing is
      unchanged
- [x] With a second state directory, `join` without `--name` lists the Library
      as `agent`
- [x] A Library made by the build on `main` before this change, on the agent's
      MinIO, with a mapped folder synced, keeps what this device put on disk
      when the new build first opens it: a following `sync` and `fetch` report
      no file as one this device did not put there
- [ ] `drive-round-trip-it.sh` and `drive-index-layout-it.sh` pass against a
      real Drive (needs a person for the consent)


