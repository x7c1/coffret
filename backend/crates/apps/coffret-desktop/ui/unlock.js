// The Passphrase window: which Library to open, and its Passphrase. The shell
// handed the list of Libraries over before this ran; the one call this page
// makes is `open_library`, and what comes back on a refusal is the sentence to
// show.
"use strict";

const form = document.getElementById("unlock");
const library = document.getElementById("library");
const passphrase = document.getElementById("passphrase");
const open = document.getElementById("open");
const refusal = document.getElementById("refusal");

const libraries = window.COFFRET_LIBRARIES || [];
for (const name of libraries) {
  const option = document.createElement("option");
  option.value = name;
  option.textContent = name;
  library.append(option);
}
if (libraries.length === 0) {
  refusal.textContent = "There is no Library on this device.";
  open.disabled = true;
} else if (libraries.length > 1) {
  // Nothing chosen for the person where there is a choice to make.
  library.selectedIndex = -1;
}

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  open.disabled = true;
  refusal.textContent = "";
  try {
    await window.__TAURI__.core.invoke("open_library", {
      name: library.value,
      passphrase: passphrase.value,
    });
  } catch (refused) {
    refusal.textContent = String(refused);
    passphrase.focus();
  } finally {
    passphrase.value = "";
    open.disabled = false;
  }
});
