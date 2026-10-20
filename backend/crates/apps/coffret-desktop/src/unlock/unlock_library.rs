use coffret_desktop_core::{refusal_evidence, refusal_line, EnteredPassphrase};
use coffret_device::open_library as reopen;
use coffret_server::Unlocked;
use tauri::State;

use super::Desktop;

/// Unlocks the Library this app serves, in place, with `passphrase`: what the
/// Passphrase window does in *unlock* mode (spec: DK-1).
///
/// The Library is reopened the way it was opened at launch — the device crate's
/// `open_library`, handed the Passphrase in the same closure — and what that
/// produced is handed to the running server, which holds it until the next lock
/// and arms that lock afresh. The Passphrase is taken here, in the desktop
/// app's own window, and never by the server's routes or the explorer's page
/// (spec: DK-1, DK-10, LA-3, LA-6).
///
/// A refusal is answered with one line — the Passphrase does not open the
/// Library, most often ([`refusal_line()`]) — with its log-safe rendering in
/// the log ([`refusal_evidence()`]), and the window stays for another try.
/// Success is answered and nothing more: the page asks for the window to be
/// hidden once it has heard it ([`super::dismiss_window()`]), and the explorer
/// hears of it in its next answer about what the server is doing.
#[tauri::command]
pub async fn unlock_library(
    desktop: State<'_, Desktop>,
    passphrase: EnteredPassphrase,
) -> Result<(), String> {
    let passphrase = passphrase.into_passphrase();

    let _turn = desktop.opening.lock().await;
    let served = desktop
        .served()
        .cloned()
        .ok_or("No Library is open in this app to unlock.")?;
    if served.holds_library() {
        // A press that waited behind the unlock that landed.
        return Ok(());
    }

    let name = served.name.clone();
    let reopened = desktop
        .runtime
        .spawn(async move { reopen(&name, move || Ok(passphrase)).await })
        .await;
    let library = match reopened {
        Ok(Ok(library)) => library,
        Ok(Err(refused)) => {
            let refused = anyhow::Error::new(refused);
            tracing::warn!(
                error = %refusal_evidence(&refused),
                "the Library could not be unlocked from the Passphrase window"
            );
            return Err(refusal_line(&refused));
        }
        Err(panicked) => {
            tracing::error!(error = %panicked, "unlocking the Library stopped without an answer");
            return Err("Unlocking the Library stopped without an answer.".to_owned());
        }
    };
    match served.unlock(library) {
        Ok(Unlocked::Now) => {
            tracing::info!(operation = "unlock", "the Library was unlocked in place")
        }
        Ok(Unlocked::Already) => {}
        Err(refused) => {
            tracing::warn!(
                error = %refusal_evidence(&refused),
                "the Library the Passphrase reopened is not the one being served"
            );
            return Err(refusal_line(&refused));
        }
    }
    Ok(())
}
