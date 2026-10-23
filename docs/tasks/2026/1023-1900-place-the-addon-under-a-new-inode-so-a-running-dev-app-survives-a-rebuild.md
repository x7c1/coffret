---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make check && test -x scripts/desktop-addon-test.sh && grep -q "desktop-addon-test" Makefile'
assignee: null
branch: task/1023-1900-place-the-addon-under-a-new-inode-so-a-running-dev-app-survives-a-rebuild
created_at: 2026-10-23T10:00:00Z
updated_at: 2026-10-23T21:20:00Z
---

# fix(desktop): place the addon under a new inode, so a running development app survives a rebuild

## Overview

`make napi-addon` (Makefile, the `napi-addon` target) copies the built
`libcoffret_napi.dylib` / `.so` to `backend/target/release/coffret.node` with
`cp`, which overwrites the file in place under the same inode. On macOS a
development app that has the addon loaded then holds a mapping whose file
changed under it; the next app started from that file was killed by the kernel
(`cs_mtime != mtime`, SIGKILL), and the running app loses the addon it is
executing. Linux is more forgiving but the same overwrite is wrong there too.

### Decision (already made; implement it)

- Move the placing of the addon out of the Makefile recipe into
  `scripts/desktop.sh` as a subcommand (`place-addon`), documented in the
  script's header like its siblings. It copies the built library to a
  temporary name beside the target and renames it over `coffret.node`
  (`mv`, atomic on the same filesystem), so the file at that path is a new
  inode and whatever has the old one mapped keeps it until it exits.
- `napi-addon` calls the subcommand. Nothing else about the target changes.
- Add `scripts/desktop-addon-test.sh`, after the pattern of
  `scripts/desktop-dev-sandbox-test.sh`: in a temporary directory, place a
  fake library twice through the subcommand and assert that the inode of
  `coffret.node` differs between the two placements and that the content is
  the second library's. Wire it into `make check` as `desktop-addon-test`
  beside `desktop-dev-sandbox-test`.

### Out of scope

- The installed app's bundle (`desktop.sh build` makes a fresh bundle; no
  in-place overwrite there).
- Signing of the development addon.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `scripts/desktop.sh place-addon` renames a freshly copied file over `backend/target/release/coffret.node`; `scripts/desktop-addon-test.sh` (executable, wired into `make check` as `desktop-addon-test` — both gates appended to `check_command`) shows two placements give two inodes and the latest content.
- [x] `make napi-addon`'s recipe calls the subcommand and no longer runs `cp` onto `coffret.node` directly.

### Before merge (verified outside the check command)

- [x] On this macOS machine: `make desktop-dev-build` twice in a row, `stat -f %i backend/target/release/coffret.node` after each, and the two inodes differ. The merging session records the two values in the PR.
- [x] On this macOS machine: `make desktop-dev` starts, the app shows its tray icon and issues its Passphrase window (`identity.ts` sets `trayTemplate` on macOS and the development name "Coffret (dev)"); the explorer is served only after the Passphrase, which the merging session does not enter. The merging session records the log lines in the PR.
- [ ] Needs a person: with the development app running, run `make desktop-dev-build` again and confirm the running app keeps working and the next start is not killed; and that the menu-bar icon follows light and dark appearance and its tooltip reads "Coffret (dev)". The person records the result in the PR.
