---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "desktop-dev-sandbox-test" Makefile && grep -rq "apparmor_restrict_unprivileged_userns" scripts/ && grep -rq "apparmor_restrict_unprivileged_userns" docs/guides/'
assignee: null
branch: task/1023-0356-say-how-to-let-the-development-app-sandbox-itself-on-linux
created_at: 2026-10-22T18:56:09Z
updated_at: 2026-10-22T19:40:00Z
---

# fix(desktop): say how to let the development app start Chromium's sandbox on Ubuntu 24.04 and later

## Overview

`make desktop-dev` does not start on Ubuntu 24.04 and later (seen on 26.04, 2026-10-23). There `kernel.apparmor_restrict_unprivileged_userns` is `1`, so only a program an AppArmor profile allows may create user namespaces, which Chromium's sandbox needs. The installed app has such a profile (`/opt/Coffret/resources/apparmor-profile`, installed as `/etc/apparmor.d/coffret-desktop`, see `docs/guides/install/ubuntu.md#the-sandbox`), but it names `/opt/Coffret/coffret-desktop` alone. The development app runs `frontend/node_modules/.pnpm/electron@<version>/node_modules/electron/dist/electron`, which no profile names, and whose `chrome-sandbox` is not setuid root. Chromium then aborts with SIGTRAP:

```
FATAL:setuid_sandbox_host.cc(166)] The SUID sandbox helper binary was found, but is not configured correctly. ... You need to make sure that .../electron/dist/chrome-sandbox is owned by root and has mode 4755.
```

The message points a developer at making `chrome-sandbox` setuid root, which `pnpm install` undoes, or toward `--no-sandbox`, which the app never takes. The environments guide tells a developer to reproduce a bug with `make desktop-dev`, so every developer on a current Ubuntu meets this on the first run.

A profile of the installed app's kind, naming the checkout's Electron binary, lets the development app start with its sandbox. It was confirmed by hand on the development machine:

```
abi <abi/4.0>,
include <tunables/global>

profile coffret-desktop-dev <checkout>/frontend/node_modules/.pnpm/electron@*/node_modules/electron/dist/electron flags=(unconfined) {
  userns,
}
```

### Decisions (already made; implement them)

1. **`scripts/desktop.sh dev` checks before it starts the app, on Linux only.** It refuses to start, with a message, when all of these hold:
   - `/proc/sys/kernel/apparmor_restrict_unprivileged_userns` exists and reads `1`;
   - the checkout's `chrome-sandbox` is not owned by root with the setuid bit;
   - no profile under `/etc/apparmor.d/` attaches to this checkout's Electron binary. Decide this by reading the profiles' text, without root. A profile attaches when its attachment path, read as an AppArmor glob (at least `*` within one path component), matches the binary's path.

   Where the file under `/proc` is missing or reads `0` (Ubuntu 22.04, other distributions, macOS), the check passes without reading anything else.
2. **The message tells the developer what to run.** It names the cause in one sentence, then prints the profile above with this checkout's absolute path filled in, and the two commands that install and load it (`sudo tee /etc/apparmor.d/coffret-desktop-dev`, `sudo apparmor_parser -r /etc/apparmor.d/coffret-desktop-dev`). The profile uses `electron@*`, so it survives an Electron upgrade. Name the profile after the checkout only if two checkouts would otherwise need the same file name. Say that a second checkout or worktree needs its own line or profile.
3. **Nothing in `make` runs `sudo` or writes under `/etc`.** The script prints and stops; the developer decides. Do not suggest `--no-sandbox` or a setuid `chrome-sandbox` in the message.
4. **The check is testable without the host's state.** Let the script read the `/proc` file, the profile directory and the Electron binary path from environment variables that default to the real locations (only for testing, documented in the script's header). Add `scripts/desktop-dev-sandbox-test.sh`, in the style of `scripts/check-version-change-test.sh`, and a `desktop-dev-sandbox-test` target wired into `make check`. It runs the check alone (not Electron) over throwaway directories and covers each state:
   - the restriction file missing → starts;
   - the restriction `0` → starts;
   - the restriction `1`, no profile → refuses, and the message holds the checkout's Electron path and both `sudo` commands;
   - the restriction `1`, a profile whose attachment matches (with `electron@*`) → starts;
   - the restriction `1`, a profile for another path only (such as the installed app's `/opt/Coffret/coffret-desktop`) → refuses;
   - the restriction `1`, `chrome-sandbox` setuid root → starts. If a throwaway file cannot be made root-owned without root, factor the ownership test so the case can stand in for it, and say how in the test.
5. **Docs.** In `docs/guides/environments.md`, where `make desktop-dev` and its first run are described, add the requirement on Ubuntu 24.04 and later and the same profile and commands. Link to it from `docs/guides/install/ubuntu.md#the-sandbox` and from the paragraph on running an extracted `.deb`, which meets the same restriction.

Out of scope: the e2e journey's `--no-sandbox` (`scripts/e2e-electron.sh`) stays as it is. The installed app's profile and the `.deb`'s install script do not change.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] `scripts/desktop-dev-sandbox-test.sh` exists, runs as the `desktop-dev-sandbox-test` target under `make check`, and covers the six states in decision 4
- [x] `scripts/desktop.sh` reads `apparmor_restrict_unprivileged_userns` before it starts the development app
- [x] `docs/guides/environments.md` names `apparmor_restrict_unprivileged_userns` and gives the profile and the two commands
- [x] `shell-lint` passes over the new and changed scripts

### Before merge (verified outside the check command)
- [x] On the Ubuntu 26.04 development machine with `/etc/apparmor.d/coffret-desktop-dev` set aside (`sudo mv` it out and `sudo apparmor_parser -R` the profile), `make desktop-dev` refuses with the message; with the printed commands run, it starts with its sandbox. Needs a person: the commands need `sudo`.
- [ ] On macOS, `make desktop-dev` starts as before — not run before merge: no macOS machine was at hand. On macOS `/proc/sys/kernel/apparmor_restrict_unprivileged_userns` does not exist, so the check passes at its first test, the path the "restriction file missing" case of `scripts/desktop-dev-sandbox-test.sh` covers; to be confirmed the next time the app is run on macOS
