---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/0928-0732-say-which-folders-a-listing-holds-on-disk
created_at: 2026-09-28T07:32:21Z
updated_at: 2026-09-28T07:51:22Z
---

# fix: say which folders a listing's folder holds on disk, so a new folder is never made over one

## Overview

The explorer refuses a new folder's name when the listing of that path shows anything already there, because the first drop into a new folder freezes it, and a freeze takes every eligible file under the folder (PK-1, PK-17) — files that were already on disk would go into the book's Pack without anyone saying so. The listing cannot show enough for that check today. `GET /api/list` answers `folders` from the catalog (`coffret-server/src/routes/list.rs`), so for a path the Library does not hold it is always empty, and the on-disk half, `OpenLibrary::added_locally` (`coffret-device/src/add/added_locally.rs`), reads one directory and keeps only files — a child that is a folder is skipped. A folder on disk whose files all sit in subfolders (a volume kept as chapter folders, for example) therefore lists as empty, the name is accepted, and the first drop packs the chapters in with the book. The comment in `askToMake` (`frontend/packages/apps/web/src/newFolder.ts`) states this limit today.

1. **Report the child folders a mapped folder holds on disk.** Have the device's listing of a folder also return the names of its child folders that stand in the mapped folder on disk and are not folders of the Library, read through the same `MappedRoots` capability and with the same exclusions `added_locally` applies to files: coffret's scratch, the management area (EP-14), and names that spell no Entry Path (EP-1) — a name that only folds to the reserved one is refused, as it is for files. It stays one directory read, as the listing is. A symbolic link is treated as `added_locally` treats one today.
2. **Carry them on the listing's wire.** Add the names to the listing answer (`ListingDto` and the explorer's `Listing` type in `frontend/packages/gateway/api/src/`), update the answer contract and its golden file (`frontend/packages/gateway/api/src/contract/answers.json`, rewritten with `COFFRET_WRITE_CONTRACT=1`, and `contract.test.ts`), and pin the new field with a route case that plants a folder with only a subfolder of files and lists it.
3. **Refuse a new folder over one.** `askToMake` refuses the name when the listing shows files, catalog folders or on-disk folders, and its comment no longer states the limit. Add the case to the new-folder unit tests.

Decision this task makes: the explorer's folder tree does not start showing on-disk-only folders. The field exists so the new-folder check can see them; drawing them is a separate question of what a person can do with a folder the Library does not hold, and is not decided here.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A route case lists a mapped folder holding only a subfolder of files and gets that subfolder's name in the new field, and not scratch or the management area (`make check`)
- [x] The answer contract's golden file carries the field and `contract.test.ts` reads it (`make check`)
- [x] `askToMake` refuses a name whose listing shows only an on-disk folder, pinned in the new-folder unit tests (`make check`)
- [x] `make check` passes
