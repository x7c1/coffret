// The Passphrase window, in one of two modes.
//
// *Open*, as the shell starts: which Library to open, and its Passphrase. The
// shell handed the list of Libraries over before this ran; the one call this
// page makes is `open_library`.
//
// *Unlock*, once the Library being served has locked itself: the shell loads
// this same page with `?unlock=<name>`, the list is fixed to that one Library,
// and the one call is `unlock_library`, which unlocks it in place.
//
// Either way, what comes back on a refusal is the sentence to show, and the
// Passphrase field is emptied whatever the answer. On success this page asks
// for the window to be hidden (`dismiss_window`) only once it has heard the
// answer, and loads itself afresh only once that too is answered: a page
// reloaded while a call waits tears the call down, and WebKitGTK sends it again
// once the page is back — the Passphrase submitted twice. A dismissal is not an
// answer, so when the window is closed the shell empties the field instead, by
// loading this page afresh as it hides it (`src/unlock/hide_window.rs`).
"use strict";

const form = document.getElementById("unlock");
const library = document.getElementById("library");
const passphrase = document.getElementById("passphrase");
const open = document.getElementById("open");
const refusal = document.getElementById("refusal");

const unlocking = new URLSearchParams(window.location.search).get("unlock");

function offer(name) {
  const option = document.createElement("option");
  option.value = name;
  option.textContent = name;
  library.append(option);
}

if (unlocking !== null) {
  // The Library is the one being served, and nothing else can be unlocked
  // here: the list holds it alone and cannot be changed.
  offer(unlocking);
  library.disabled = true;
  open.textContent = "Unlock";
} else {
  const libraries = window.COFFRET_LIBRARIES || [];
  for (const name of libraries) {
    offer(name);
  }
  if (libraries.length === 0) {
    refusal.textContent = "There is no Library on this device.";
    open.disabled = true;
  } else if (libraries.length > 1) {
    // Nothing chosen for the person where there is a choice to make.
    library.selectedIndex = -1;
  }
}

// What the button says while the call it made is unanswered. Opening a Library
// can take a while (the Passphrase is stretched, Storage is caught up with),
// and a button that only greys out does not say the press landed.
const idle = open.textContent;
const pending = unlocking !== null ? "Unlocking…" : "Opening…";

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  open.disabled = true;
  open.textContent = pending;
  refusal.textContent = "";
  let opened = false;
  try {
    if (unlocking !== null) {
      await window.__TAURI__.core.invoke("unlock_library", {
        passphrase: passphrase.value,
      });
    } else {
      await window.__TAURI__.core.invoke("open_library", {
        name: library.value,
        passphrase: passphrase.value,
      });
    }
    opened = true;
  } catch (refused) {
    refusal.textContent = String(refused);
    open.textContent = idle;
    open.disabled = false;
    passphrase.focus();
  } finally {
    passphrase.value = "";
  }
  if (opened) {
    // The button stays disabled until the page is gone: a press while the
    // window is still up would reach a Library already open, and the explorer
    // would be opened a second time.
    try {
      await window.__TAURI__.core.invoke("dismiss_window");
    } finally {
      // Whether or not the window could be hidden: the fresh page is what
      // drops this one's script state, the submitted string included.
      window.location.reload();
    }
  }
});
