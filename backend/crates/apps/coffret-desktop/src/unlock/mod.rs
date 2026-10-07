//! The Passphrase window, and what happens when it is answered.
//!
//! The window is the shell's own page (`ui/`), not the explorer's: a list of
//! the Libraries on this device, a Passphrase field, and a line for a refusal.
//! It is in one of two modes. *Open*, as the shell starts: it makes one call,
//! [`open_library()`], and is told either nothing (the Library is open, and the
//! window goes away) or the sentence the server would have printed on a
//! terminal (and the window stays, for another try). *Unlock*, once the
//! Library being served has locked: the server's prompt or the tray's
//! *Unlock…* brings it forward ([`ask_for_passphrase()`]) with the Library
//! fixed to the one served, and its one call is [`unlock_library()`], which
//! answers the same two ways.

mod ask_for_passphrase;
pub use ask_for_passphrase::ask_for_passphrase;

mod bring_forward;
pub use bring_forward::bring_forward;

mod hide_window;
use hide_window::hide_window;

mod open_library;
// A glob, because `generate_handler!` reaches the command through the items
// `#[tauri::command]` generates beside it, not through the function alone.
pub use open_library::*;

mod open_window;
pub use open_window::open_window;

mod shell;
pub use shell::Shell;

mod unlock_library;
// A glob, for the reason `open_library`'s is.
pub use unlock_library::*;

/// The Passphrase window's label.
const WINDOW_LABEL: &str = "unlock";

/// The query that puts the window's page in *unlock* mode, naming the Library
/// to unlock. The page reads the same name (`ui/unlock.js`).
const UNLOCK_QUERY: &str = "unlock";
