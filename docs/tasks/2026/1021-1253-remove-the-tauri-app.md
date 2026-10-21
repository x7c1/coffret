---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! test -e backend/crates/apps/coffret-desktop && ! grep -qE "^(tauri|tauri-build|tauri-plugin-[a-z-]+|glib) *=" backend/Cargo.toml && ! grep -q "libwebkit2gtk" .github/workflows/ci.yml .github/workflows/toolchain-drift.yml && test -n "$(ls frontend/packages/apps/e2e/electron 2>/dev/null)"'
assignee: null
branch: task/1021-1253-remove-the-tauri-app
created_at: 2026-10-21T03:53:43Z
updated_at: 2026-10-21T04:52:16Z
---

# refactor(desktop): remove the Tauri app now that the Electron app replaces it

## Overview

The Electron app (`frontend/packages/apps/desktop`, over the `coffret-napi` addon and `coffret-desktop-core`) is now the desktop app that is built, installed and released. The Tauri crate `backend/crates/apps/coffret-desktop` is still in the workspace: it builds in `make check` and needs WebKitGTK in CI, but nothing ships it. This change removes it and everything that exists only for it, and adds an end-to-end journey that drives the Electron app itself.

### Decisions (already made; implement them)

1. **Remove the Tauri crate.**
   - Delete `backend/crates/apps/coffret-desktop` and drop it from the workspace and from `make deps`'s `APPS`.
   - Remove the workspace dependencies only it used: `tauri`, `tauri-build`, the `tauri-plugin-*` crates and `glib`, plus anything else `cargo tree` shows only it pulling in.
   - Update `Cargo.lock`.
   - In `backend/deny.toml`, remove the advisory waivers, license exceptions and target comments that were there only for Tauri's tree. Keep any that `cargo deny check` still needs; the cargo-deny job must stay green.
2. **Icons.** The generated icon sets now live under the crate (`icons/`, `icons-dev/`, `icons-tray/`).
   - Move what the Electron app and the explorer still use next to the desktop package: `packaging/` for the bundle icons, `assets/` for the tray and window icons.
   - Make `scripts/gen-icons.sh` (`make icons`) write there directly, and stop writing the Tauri-only outputs.
   - If `cargo tauri icon` stays as the SVG rasteriser, say so in the script's header. Otherwise replace it with a tool the script checks for, and name it in `docs/guides/environments.md`.
3. **The launch file.** The desktop app no longer writes launch files.
   - The development host `coffret-explorer-dev` (`make dev`, `make web`, and the browser e2e journeys) still opens the explorer in a system browser, so it keeps `coffret-explorer-host`'s `launch_file` module.
   - Rescope LA-21 in `docs/spec/loopback-access/README.md`:
     - the desktop app hands the ticket only in the fragment of the URL it loads into its own window;
     - only the development host writes an owner-only launch file, for a browser.
   - Rewrite LA-6, LA-17 and LA-18 wherever they still describe the desktop app writing files or opening a browser.
   - Remove `coffret-explorer-host` code that only the Tauri app used, if any.
4. **Wording in code and docs.** Remove or rewrite every remaining mention of the Tauri app, its webview, its IPC, its tray items or its launch file, outside `docs/tasks/` (history) and ADR-like records. This covers:
   - the server's locked refusal in `backend/crates/apps/coffret-server/src/api_error/admission.rs`, which points to "the app's tray icon": keep it true for the Electron tray;
   - `coffret-desktop-core` comments;
   - `docs/guides/environments.md`, `release.md` and `install/`;
   - `README.md`;
   - the spec, including DK-1 and DK-7, where the webview-and-IPC path no longer exists;
   - the concept docs `docs/concepts/passphrase/`, `master-key/` and `library/`, which still name a webview-and-IPC path. Name only the Electron app's door path.
5. **CI.**
   - Drop the WebKitGTK / GTK / appindicator / rsvg / xdo apt packages and their cache steps from `ci.yml` and `toolchain-drift.yml`. Keep any package another crate still needs; check `libssl-dev`.
   - Make sure the `backend` job still builds and tests the workspace.
6. **An Electron end-to-end journey.** Add `frontend/packages/apps/e2e/electron/`: a Playwright `_electron` suite against the unpackaged app, built with `make napi-addon` plus the desktop package. It uses a throwaway state directory and the S3 / MinIO setup that `scripts/e2e-it.sh` already provides. The suite:
   - opens a Library through the Passphrase window;
   - sees the explorer list it;
   - takes a real-path drop of a file through CDP's `Input.dispatchDragEvent`, and sees it land;
   - after an idle lock (`COFFRET_IDLE_MINUTES=1`, or a shortened interval if the core allows one for tests), sees the Passphrase window come back and unlocks in place;
   - quits and sees no process left.

   Wire it into `make` as its own target and into CI's `e2e` job on Linux under `xvfb-run`. Passing `--no-sandbox` is acceptable for this test run only; never in the app or the bundle. Remove `ELECTRON_SKIP_BINARY_DOWNLOAD` from the e2e job only, since it now runs Electron. The browser journeys stay as they are.

### Out of scope

- Changing the Electron app's behaviour.
- Signing with a Developer ID.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `backend/crates/apps/coffret-desktop` no longer exists (gated). `backend/Cargo.toml` declares no `tauri*` or `glib` workspace dependency (gated).
- [x] `ci.yml` and `toolchain-drift.yml` install no WebKitGTK package (gated).
- [x] `frontend/packages/apps/e2e/electron/` exists (gated) and type-checks and lints with the rest of the frontend (`make check`).
- [x] `make spec-rule-ids` and `make spec-citations` pass after the LA, DK and concept edits. No `spec:` citation points to a rule that no longer says what the code relies on.
- [x] The rest of `make check` passes without the Tauri crate: `make deps`, the Rust build, tests, clippy and doc, and the frontend.

### Before merge (verified outside the check command)

- [x] This PR's CI is green, including `cargo-deny` with the trimmed `deny.toml`, the `e2e` job running the new Electron journey under xvfb, and the Bundle workflow.
- [x] `make icons` regenerates the moved icon sets byte-identically, or with only the intended path changes, on macOS.
- [x] On macOS, the new Electron journey passes locally against the agent's MinIO.
