//! The Passphrase window, and what happens when it is answered.
//!
//! The window is the app's own page (`ui/`), not the explorer's: a list of
//! the Libraries on this device, a Passphrase field, and a line for a refusal.
//! It is in one of two modes. *Open*, as the app starts: it makes one call,
//! [`open_library()`], and is told either nothing (the Library is open) or the
//! sentence to show (and the window stays, for another try). *Unlock*, once the
//! Library being served has locked: the server's prompt or the tray's
//! *Unlock…* brings it forward ([`ask_for_passphrase()`]) with the Library
//! fixed to the one served, and its one call is [`unlock_library()`], which
//! answers the same two ways. A refusal is shown as one line
//! ([`refusal_line()`]), and its log-safe rendering goes to the log
//! ([`refusal_evidence()`]).
//!
//! Neither call hides the window. The answer has to reach the page first: a
//! window hidden and reloaded while the call waits tears the call down, and
//! WebKitGTK sends it again, Passphrase and all, once the page is back. So the
//! page, having heard that the Library is open, asks for the window to go
//! ([`dismiss_window()`]), and once that too is answered loads itself afresh.
//!
//! Whatever brings the window forward, the Passphrase's path is the same: the
//! field, the page's script, Tauri's IPC, and then one of those two calls,
//! which receives it as an [`EnteredPassphrase`] — already the type that wipes
//! it. DK-7's claim begins there. The copies before it are the webview's and
//! the IPC's, which coffret cannot overwrite; what the app does about them is
//! keep them short-lived: the page empties its field after every submission,
//! and every hiding of the window loads the page afresh — the page itself
//! after an open or an unlock that landed, and the app ([`hide`]) when the
//! window is closed — so nothing typed is left behind in a window nobody is
//! looking at.

mod ask_for_passphrase;
pub use ask_for_passphrase::ask_for_passphrase;

mod bring_forward;
pub use bring_forward::bring_forward;

mod entered_passphrase;
pub use entered_passphrase::EnteredPassphrase;

mod dismiss_window;
// A glob, for the reason `open_library`'s is.
pub use dismiss_window::*;

mod hide_window;
use hide_window::hide;

mod open_library;
// A glob, because `generate_handler!` reaches the command through the items
// `#[tauri::command]` generates beside it, not through the function alone.
pub use open_library::*;

mod open_window;
pub use open_window::open_window;

mod refusal_line;
use refusal_line::{refusal_evidence, refusal_line};

mod desktop;
pub use desktop::Desktop;

mod unlock_library;
// A glob, for the reason `open_library`'s is.
pub use unlock_library::*;

/// The Passphrase window's label.
const WINDOW_LABEL: &str = "unlock";

/// The query that puts the window's page in *unlock* mode, naming the Library
/// to unlock. The page reads the same name (`ui/unlock.js`).
const UNLOCK_QUERY: &str = "unlock";
