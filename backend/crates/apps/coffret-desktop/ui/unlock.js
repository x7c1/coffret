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
// Passphrase field is emptied whatever the answer. A dismissal is not an
// answer, so the shell empties it too, by loading this page afresh whenever it
// hides the window (`src/unlock/hide_window.rs`).
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

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  open.disabled = true;
  refusal.textContent = "";
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
  } catch (refused) {
    refusal.textContent = String(refused);
    passphrase.focus();
  } finally {
    passphrase.value = "";
    open.disabled = false;
  }
});
