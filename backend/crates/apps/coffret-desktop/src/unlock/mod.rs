//! The Passphrase window, and what happens when it is answered.
//!
//! The window is the shell's own page (`ui/`), not the explorer's: a list of
//! the Libraries on this device, a Passphrase field, and a line for a refusal.
//! It makes one call, [`open_library()`], and is told either nothing (the
//! Library is open, and the window goes away) or the sentence the server would
//! have printed on a terminal (and the window stays, for another try).

mod bring_forward;
pub use bring_forward::bring_forward;

mod open_library;
// A glob, because `generate_handler!` reaches the command through the items
// `#[tauri::command]` generates beside it, not through the function alone.
pub use open_library::*;

mod open_window;
pub use open_window::open_window;

mod shell;
pub use shell::Shell;

/// The Passphrase window's label.
const WINDOW_LABEL: &str = "unlock";
