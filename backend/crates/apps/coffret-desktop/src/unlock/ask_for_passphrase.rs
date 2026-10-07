use tauri::{AppHandle, Manager, Url};

use super::{bring_forward, Shell, UNLOCK_QUERY, WINDOW_LABEL};

/// Puts the Passphrase window in front in *unlock* mode, for the Library this
/// shell serves — what the server's prompt and the tray's *Unlock…* both do
/// once the Library has locked (spec: DK-1).
///
/// The mode is a query on the window's own page rather than a second page:
/// the page reads the Library's name from it, fixes the list to that one name,
/// and offers *Unlock* where it offered *Open*. Loading the page afresh is also
/// what leaves the Passphrase field empty. A window already showing the unlock
/// is only brought forward, so a second asking does not wipe what somebody is
/// in the middle of typing.
///
/// Nothing is asked where the Library is open, or where none is served yet: an
/// asking that arrives after another unlock already landed has nothing left to
/// ask for.
pub fn ask_for_passphrase(app: &AppHandle) {
    let shell = app.state::<Shell>();
    let Some(served) = shell.served() else {
        tracing::warn!("the Passphrase was asked for again before a Library was open");
        return;
    };
    if served.holds_library() {
        tracing::info!("the Passphrase was asked for again, and the Library is already unlocked");
        return;
    }
    let Some(window) = app.get_webview_window(WINDOW_LABEL) else {
        tracing::warn!("the Passphrase window is not there to ask in");
        return;
    };
    let showing = window.is_visible().unwrap_or(false)
        && window
            .url()
            .is_ok_and(|url| url.query_pairs().any(|(name, _)| name == UNLOCK_QUERY));
    if !showing {
        match window.url() {
            Ok(url) => {
                if let Err(error) = window.navigate(in_unlock_mode(url, &served.name)) {
                    tracing::warn!(%error, "the Passphrase window could not be put in unlock mode");
                    return;
                }
            }
            Err(error) => {
                tracing::warn!(%error, "the Passphrase window's page could not be read");
                return;
            }
        }
    }
    bring_forward(app);
}

/// The window's page, asking for the Passphrase of the Library called `name`.
fn in_unlock_mode(mut page: Url, name: &str) -> Url {
    page.set_fragment(None);
    page.query_pairs_mut()
        .clear()
        .append_pair(UNLOCK_QUERY, name);
    page
}
