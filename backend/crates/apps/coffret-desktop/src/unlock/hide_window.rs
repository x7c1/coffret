use tauri::WebviewWindow;

/// Hides `window` and loads its page afresh, which empties the Passphrase field.
///
/// The hidings the shell takes on its own come through here — a dismissal once
/// a Library is served — so that nothing typed stays in a page nobody is
/// looking at. The page empties the field itself after every submission, but a
/// dismissal is not a submission: without this, what somebody typed and then
/// thought better of would sit in the hidden page until the next asking loaded
/// it again. After an open or an unlock that landed, the page hides the window
/// and reloads itself instead ([`super::dismiss_window()`]).
///
/// Reloading drops the page's script state along with the field, the string the
/// last call was made with included. That shortens how long the webview's copies
/// live; it does not overwrite them, and nothing coffret runs can — they are
/// outside what DK-7 claims.
pub(super) fn hide(window: &WebviewWindow) {
    if let Err(error) = window.hide() {
        tracing::warn!(%error, "the Passphrase window could not be hidden");
    }
    // After hiding, so the page reloading is not seen; the same page, so the
    // window comes back in the mode it left in.
    if let Err(error) = window.reload() {
        tracing::warn!(%error, "the Passphrase window's page could not be emptied");
    }
}
