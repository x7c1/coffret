---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make shell-lint && grep -q "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER" scripts/desktop.sh && grep -q "_apt" scripts/desktop.sh'
assignee: null
branch: task/1007-1600-link-the-desktop-app-with-the-systems-toolchain-on-linux
created_at: 2026-10-07T06:44:40Z
updated_at: 2026-10-07T06:57:03Z
---

# fix(desktop): link the desktop app with the system's toolchain on Linux

## Overview

On a development machine whose `PATH` puts a Nix profile's `cc` first,
`make desktop` builds a `.deb` whose `coffret-desktop` asks for Nix's dynamic
loader (`/nix/store/…-glibc-…/lib/ld-linux-x86-64.so.2`) and carries Nix store
paths in its `RUNPATH`. Installed under `/usr/bin`, that binary does not start
at all — from the desktop, from a terminal, or from an empty environment:

    /usr/bin/coffret-desktop: error while loading shared libraries: libpango-1.0.so.0: cannot open shared object file: No such file or directory

Nix's loader does not read the host's library cache, and `RUNPATH` applies only
to an object's direct dependencies, so a library GTK or WebKitGTK needs
(`libpango-1.0.so.0`) is never found. The bundle the release workflow builds on
a GitHub runner has no Nix and is unaffected; this is the local build.

Rebuilding with `CC=/usr/bin/cc CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=/usr/bin/cc`
gives a binary that asks for `/lib64/ld-linux-x86-64.so.2` and whose libraries
all resolve from an empty environment (`env -i /lib64/ld-linux-x86-64.so.2 --list <binary>`).

**1. Link with the system's toolchain.** In `scripts/desktop.sh`, on Linux,
build both the bundled app (`build`) and the development app (`dev-build`) with
`CC` and `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER` set to the system's C
compiler (`/usr/bin/cc`), overriding whatever `PATH` resolves. The app links
against the distribution's GTK and WebKitGTK, so it must be loaded by the
distribution's loader. Fail with a clear sentence when `/usr/bin/cc` is missing
(name the package, `build-essential`). Leave macOS as it is. Do not change how
the server and the command line are built: they do not load GTK.

**2. Refuse a binary the system cannot load.** After a Linux build, before
bundling is reported done (and before `install`), check the built
`coffret-desktop`: its program interpreter (`readelf -l`) is under `/lib64` or
`/lib`, not `/nix/store`, and `env -i <that interpreter> --list <binary>`
succeeds. Fail with a sentence naming the interpreter found otherwise, so a
broken `.deb` is never installed again. `readelf` comes from binutils, which
`build-essential` brings.

**3. Install without apt's sandbox notice.** `sudo apt install --reinstall`
of a `.deb` under the home directory prints "Download is performed unsandboxed
as root as file … couldn't be accessed by user '_apt'". Copy the `.deb` into a
fresh temporary directory that `_apt` can read (mode 0755 directory, 0644 file),
install from there, and remove it afterwards (a `trap`).

**4. Docs.** `docs/guides/environments.md`'s paragraph on building the desktop
app says what it needs; add that on Linux it links with the system's `cc`
(`build-essential`), whatever `PATH` puts first.

Out of scope: the release workflow (its runners have no Nix), macOS, the
server's and the command line's builds.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] `scripts/desktop.sh` sets `CC` and `CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER` to the system's compiler for the Linux builds, and passes `make shell-lint`
- [x] The interpreter check refuses a binary whose interpreter is under `/nix/store` or whose libraries do not resolve from an empty environment
- [x] `install` copies the `.deb` to a temporary directory readable by `_apt` and removes it afterwards
- [x] `docs/guides/environments.md` says the Linux build links with the system's `cc`
- [x] `make check` passes

### Before merge (verified outside the check command)
- [ ] On this machine (Nix profile first on `PATH`), `make desktop-build` produces a `.deb` whose binary asks for `/lib64/ld-linux-x86-64.so.2` and resolves from an empty environment, and `make desktop-dev-build` likewise — an agent runs both and reports the interpreter lines
- [ ] Needs a person: `make desktop` installs without the `_apt` notice, and Coffret from the Activities overview opens its window
