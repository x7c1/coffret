---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rqF "is not a page" docs/spec docs/concepts && grep -qE "explorer.s page" docs/spec/device-key-custody/README.md'
assignee: null
branch: task/1008-0147-say-where-the-passphrases-erasure-guarantee-begins
created_at: 2026-10-07T16:47:31Z
updated_at: 2026-10-07T17:30:25Z
---

# fix(desktop): say where the Passphrase's erasure guarantee begins, and keep a dismissed entry out of the hidden window

## Overview

The desktop app takes the Passphrase in a window of its own: a small page the
app ships (`backend/crates/apps/coffret-desktop/ui/`), shown in the app's
webview. The value lives in the page as the input field's value and a
JavaScript string, crosses Tauri's IPC as a serialized message, arrives in the
command handlers `open_library` and `unlock_library`
(`src/unlock/open_library.rs`, `src/unlock/unlock_library.rs`) as a `String`,
and is moved into `Passphrase` with `into_bytes` (a move, not a copy).

The spec and concepts describe this path more strongly than it is:

- DK-1 (`docs/spec/device-key-custody/README.md`) and the Passphrase concept
  (`docs/concepts/passphrase/README.md`) say the prompt "is not a page". The
  window *is* a page, rendered by the app; what it is not is the explorer's
  page in the system browser, which never carries a secret (LA-3, LA-6).
- DK-7 says secret material "is never copied into a buffer outside" its
  erasing type, and lists the Passphrase in the secret-bearing inventory. That
  holds from the moment coffret's own command handler holds the value. The
  copies the webview and the IPC make before that — the field's value, the
  JavaScript string, the serialized message — are outside anything coffret can
  overwrite.

The decision recorded for this is: the guarantee begins where coffret's own
code receives the value; the webview and IPC copies are outside it and are
shortened on a best-effort basis; the spec and concepts say so rather than
claiming more.

**1. Spec.** In DK-1, name the two surfaces: the explorer's page in the system
browser, which never takes or carries a secret, and the desktop app's own
window, which is the one surface that takes the Passphrase. Replace "is not a
page" with wording that says the prompt is the desktop app's window and not
the explorer's page. Add that whatever brings the window forward — a press of
*unlock*, the tray, or the explorer asking on its own once it sees the Library
locked — only decides when the window is shown; the Passphrase's path is the
same. In DK-7, state where the claim begins for a secret entered in the
desktop app's window: once the app's command handler holds it, it is in the
erasing type with no copy outside it; copies the webview and its IPC made
before that are outside the claim, and the app shortens their life (below)
without guaranteeing their erasure. DK-10 keeps its meaning; adjust only if
it repeats "not a page".

**2. Concepts.** Bring `docs/concepts/passphrase/README.md` (and
`master-key/README.md` if it states the same path) into line with the spec:
the same two surfaces, the same limit. Do not restate the rule; cite it.

**3. The receiving boundary in code.** Make the hand-over explicit so no
intermediate `String` of the Passphrase exists in coffret's own code: have the
two commands receive a type that deserializes straight into the erasing type
(for example a newtype over `Passphrase` with its own `Deserialize`), or show
that the current `String` → `into_bytes` move already leaves no copy and say
so in the handler's doc comment. Whatever type ends up holding the secret
bytes joins the secret-bearing inventory and its assertion (DK-7's testable
half). No `Debug`/`Display` that prints the secret.

**4. Shorten the webview's copy.** The field is emptied after every submit
(`ui/unlock.js`, `finally`). It is not emptied when the window is dismissed
with something typed but not submitted: closing it once a Library is served
hides it (`src/unlock/open_window.rs`), and the typed text stays in the hidden
page until the next asking reloads it. Empty the field whenever the window is
hidden — on dismissal and on success — for example by reloading the window's
page or clearing the field from the shell when it hides the window. Keep the
existing rule that a second asking while the window is showing does not wipe
what somebody is in the middle of typing.

Out of scope: a native (non-webview) prompt, Recovery Code entry in the window
(no such window exists yet), and the explorer's automatic asking itself.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] No spec or concept doc says the Passphrase prompt "is not a page"; DK-1 names the explorer's page and the desktop app's window as distinct surfaces
- [x] DK-7 states where its claim begins for a secret entered in the desktop app's window and that webview/IPC copies are outside it
- [x] The commands that receive the Passphrase hold it in the erasing type from the moment they receive it, and any new type holding it is in the secret-bearing inventory assertion
- [x] Hiding the Passphrase window empties its field, covered by a test where the shell code allows one
- [x] `make check` passes

### Before merge (verified outside the check command)
- [ ] Needs a person: under `make desktop-dev`, after an idle lock, typing a few characters into the Passphrase window and closing it, then asking to unlock again, shows an empty field
