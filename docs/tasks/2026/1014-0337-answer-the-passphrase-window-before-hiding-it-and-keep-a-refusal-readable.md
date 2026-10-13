---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -q "hide_window(" backend/crates/apps/coffret-desktop/src/unlock/open_library.rs && ! grep -q "hide_window(" backend/crates/apps/coffret-desktop/src/unlock/unlock_library.rs && grep -q "overflow-wrap" backend/crates/apps/coffret-desktop/ui/unlock.css'
assignee: null
branch: task/1014-0337-answer-the-passphrase-window-before-hiding-it-and-keep-a-refusal-readable
created_at: 2026-10-13T18:37:00Z
updated_at: 2026-10-14T02:24:00Z
---

# fix(desktop): answer the Passphrase window before hiding it, and keep a refusal readable in it

## Overview

Two defects in the desktop app's Passphrase window
(`backend/crates/apps/coffret-desktop`), both seen on Linux.

### 1. The command is answered after the window is hidden and reloaded, so it runs twice

`open_library` (`src/unlock/open_library.rs:21-80`) opens the Library,
starts the server, shows the tray, then calls `hide_window(&app)` (`:77`),
opens the explorer, and only then returns `Ok(())`. `hide_window`
(`src/unlock/hide_window.rs:28-37`) hides the window and **reloads the page**
so no entered text survives in the hidden window. The reload tears down the
page whose `invoke` is still waiting for the answer; WebKitGTK then re-sends
the interrupted IPC request by `postMessage` once the page is back, and the
command runs a second time with the same Passphrase (seen with `strace` on
Linux). The replay takes the "already open" branch (`:31-35`) and opens the
explorer again, which is the two tabs the user saw. `unlock_library`
(`src/unlock/unlock_library.rs`) has the same shape at `:34-38` (the waiter
path) and `:66`.

Make the answer reach the page before the window goes away, and make the
hide a step the page asks for once it has the answer:

- `open_library` and `unlock_library` no longer call `hide_window`. They
  return `Ok(())` once the Library is open (the explorer may still be opened
  from Rust before returning: that is a separate process and does not touch
  the window).
- Add a Tauri command `dismiss_window` (register it beside the two in
  `src/main.rs:66-69`) that performs today's `hide()` — hide, then reload —
  for the window labelled `unlock`. The page's submit handler
  (`ui/unlock.js:52-74`) awaits `invoke("open_library" | "unlock_library")`
  and, on success, invokes `dismiss_window`; on refusal it keeps showing the
  refusal and focusing the field, as today. The `finally` that empties the
  field keeps running on both paths; with the hide now after the answer, it
  actually runs on success too.
- The close-request path (`src/unlock/open_window.rs:24-39`) and
  `ask_for_passphrase` keep using `hide()` / `navigate` from Rust; they are
  not answers to an IPC call.
- Keep the `opening` mutex (`src/unlock/shell.rs:15-18`) as the guard
  against a second press. Update the doc comments that describe the order
  of events (`src/unlock/mod.rs:5-21`, the two commands' docs, `hide_window.rs`,
  and the header of `unlock.js:11-14`): the page is reloaded on dismissal,
  which the page asks for after it has heard the answer.

The gate in `check_command` greps that neither command file calls
`hide_window(` any more.

### 2. A refusal is the whole error chain with a path, in a box that cannot wrap

A wrong Passphrase is shown as `format!("{refused:#}")`
(`open_library.rs:58`, `unlock_library.rs:49,63`): the full chain, for the
wrong-Passphrase case
`the Master Key at <state dir>/libraries/<name>/master-key.cfmk did not open; the Passphrase may not be the one it was written under: message failed authentication`.
The window is 380×260 and not resizable (`src/unlock/open_window.rs:13-22`),
and `#refusal` in `ui/unlock.css:33-37` has no wrapping rule, so the path,
which has no spaces, pushes the sentence off the right edge and the person
reads nothing.

- **What is shown.** The window shows one sentence a person can act on,
  and the refusal goes to the log at `warn` in its log-safe rendering (the
  device error's redacted form, EL-1 / EL-4: no local path, no Library name,
  no wholesale chain). For the refusals a person
  causes, say it plainly: a wrong Passphrase → `That is not this Library's
  Passphrase.` (the device error is `coffret_device::Error::MasterKeyNotUnlocked`;
  match it where the chain is still typed, before it becomes `anyhow`, or
  downcast). For every other refusal keep the outermost sentence of the chain
  (`{refused}` rather than `{refused:#}`), so a bind failure or a held server
  lock still says what happened without the causes appended. Put this in one
  function in its own module (`src/unlock/refusal_line.rs` or similar) with
  unit tests, used by both commands, so the wording lives in one place.
- **How it is laid out.** `#refusal` gets `overflow-wrap: anywhere` (the
  gate in `check_command` greps for it) and a `max-height` of about four
  lines with `overflow-y: auto`, so a longer sentence scrolls instead of
  growing the form past the window. The window's size stays 380×260: the
  form has to fit the sentence, not the other way round.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Neither `src/unlock/open_library.rs` nor `src/unlock/unlock_library.rs`
      calls `hide_window(` (gates in `check_command`); a `dismiss_window`
      command is registered in `src/main.rs` and invoked from `ui/unlock.js`
      after a successful `open_library` / `unlock_library`
- [x] Unit tests for the refusal line: a `MasterKeyNotUnlocked` error renders
      `That is not this Library's Passphrase.` with no path in it; another
      error with a cause chain renders its outermost sentence only
- [x] `ui/unlock.css` `#refusal` has `overflow-wrap` and a bounded height
      with vertical scrolling (gate greps `overflow-wrap`)
- [x] `make check` passes (coffret-desktop is a workspace member, so it is
      built, tested and clippied without the `embed-web` feature)

### Before merge (verified outside the check command)

- [x] `make desktop-dev-build` succeeds on this macOS machine, and the
      development app started with `COFFRET_STATE_DIR` pointing at the agent's
      state directory shows its window and the Library in its list (an agent
      on this machine runs it; the window is only looked at, not operated)
- [ ] On Linux, in the development app: submitting the Passphrase opens the
      explorer once, and the window is hidden after the answer (one
      `open_library` in the log); a wrong Passphrase shows the one sentence,
      wrapped, and the field is focused — needs a person: the replay was
      observed on WebKitGTK and the fix is judged there
