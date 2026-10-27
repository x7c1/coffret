// The Passphrase window, served by the door it submits to.
//
// The shell loads this page from the door's URL, with the window's token in
// the fragment: `#door=<token>`. The token is taken out of the address before
// anything else, so that nothing that reads the address afterwards sees it.
// The page then asks the door what this window is for (`GET /window`), in one
// of two modes:
//
// *Open*, as the app starts: which Library to open, and its Passphrase. Each
// Library is listed under its name and chosen by its ID, since two Libraries
// on one device may carry the same name. One nothing has named yet is said to
// have no name and shown by its ID, and one held in more than one directory
// is listed once and marked as needing attention.
//
// *Unlock*, once the Library being served has locked itself: the list holds
// that one Library alone and cannot be changed, and the Library is unlocked in
// place.
//
// Either way the one call is `POST /submit`, to the origin this page came
// from, with the token in `Authorization`. What comes back on a refusal is the
// sentence to show, and whether the token has ended with it: after a few
// refused tries the window has to be opened again from the app. The Passphrase
// field is emptied whatever the answer.
"use strict";

const form = document.getElementById("unlock");
const library = document.getElementById("library");
const passphrase = document.getElementById("passphrase");
const open = document.getElementById("open");
const refusal = document.getElementById("refusal");

const token = new URLSearchParams(window.location.hash.slice(1)).get("door");
history.replaceState(null, "", window.location.pathname);

const AGAIN = "This window can no longer take the Passphrase. Open it again from the app.";

let mode = null;

// Nothing more can be submitted from this window: what it says is the last
// thing it says.
function end(sentence) {
  refusal.textContent = sentence;
  library.disabled = true;
  passphrase.disabled = true;
  open.disabled = true;
}

// What a Library is listed as: its name, or that it has none yet beside its
// ID, never its ID where the name goes; and a mark where it needs attention.
function labelOf(listed) {
  const named = listed.name == null ? `(no name yet) ${listed.id}` : listed.name;
  return listed.needs_attention ? `${named} — needs attention: in more than one directory` : named;
}

function offer(value, label) {
  const option = document.createElement("option");
  option.value = value;
  option.textContent = label;
  library.append(option);
}

// A request to the door this page came from, presenting the window's token.
function call(path, init) {
  return fetch(path, {
    ...init,
    headers: { ...(init.headers || {}), Authorization: `Door ${token}` },
    cache: "no-store",
    credentials: "omit",
    redirect: "error",
  });
}

// The refusal an answer carries, as `{ message, ended }`.
async function refusalOf(answer) {
  try {
    const body = await answer.json();
    return { message: String(body.message), ended: Boolean(body.ended) };
  } catch {
    return { message: "The app did not answer.", ended: false };
  }
}

async function load() {
  open.disabled = true;
  if (!token) {
    end("Open this window from the app.");
    return;
  }
  let answer;
  try {
    answer = await call("/window", { method: "GET" });
  } catch {
    end("The app did not answer.");
    return;
  }
  if (!answer.ok) {
    const refused = await refusalOf(answer);
    end(refused.ended ? AGAIN : refused.message);
    return;
  }
  const described = await answer.json();
  mode = described.mode;
  if (mode === "unlock") {
    if (!described.library) {
      end("No Library is open in this app to unlock.");
      return;
    }
    offer(described.library.id, labelOf(described.library));
    library.disabled = true;
    open.textContent = "Unlock";
  } else {
    const libraries = described.libraries || [];
    for (const listed of libraries) {
      offer(listed.id, labelOf(listed));
    }
    if (libraries.length === 0) {
      end("There is no Library on this device.");
      return;
    }
    if (libraries.length > 1) {
      // Nothing chosen for the person where there is a choice to make.
      library.selectedIndex = -1;
    }
  }
  open.disabled = false;
  passphrase.focus();
}

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (mode === null) {
    return;
  }
  // What the button says while the call it made is unanswered. Opening a
  // Library can take a while (the Passphrase is stretched, Storage is caught
  // up with), and a button that only greys out does not say the press landed.
  const idle = open.textContent;
  open.disabled = true;
  open.textContent = mode === "unlock" ? "Unlocking…" : "Opening…";
  refusal.textContent = "";
  const body =
    mode === "unlock"
      ? { passphrase: passphrase.value }
      : { library: library.value, passphrase: passphrase.value };
  let answer = null;
  try {
    answer = await call("/submit", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body),
    });
  } catch {
    answer = null;
  } finally {
    passphrase.value = "";
  }
  open.textContent = idle;
  if (answer === null) {
    refusal.textContent = "The app did not answer.";
    open.disabled = false;
    passphrase.focus();
    return;
  }
  if (answer.ok) {
    // The token is spent; the shell closes the window. Until it does, nothing
    // more can be sent from it.
    end(mode === "unlock" ? "The Library is unlocked." : "The Library is open.");
    refusal.className = "done";
    return;
  }
  const refused = await refusalOf(answer);
  if (refused.ended) {
    end(`${refused.message} ${refused.message === AGAIN ? "" : AGAIN}`.trim());
    return;
  }
  refusal.textContent = refused.message;
  open.disabled = false;
  passphrase.focus();
});

load();
