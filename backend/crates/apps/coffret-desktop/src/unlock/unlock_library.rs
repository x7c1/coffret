use coffret_device::{open_library as reopen, Passphrase};
use coffret_server::Unlocked;
use tauri::{AppHandle, State};

use super::{hide_window, Shell};

/// Unlocks the Library this shell serves, in place, with `passphrase`: what the
/// Passphrase window does in *unlock* mode (spec: DK-1).
///
/// The Library is reopened the way it was opened at launch — the device crate's
/// `open_library`, handed the Passphrase in the same closure — and what that
/// produced is handed to the running server, which holds it until the next lock
/// and arms that lock afresh. The Passphrase is taken here, in the shell's own
/// window, and never by the server's routes or the explorer's page (spec:
/// DK-10, LA-3, LA-6).
///
/// A refusal is answered with the sentence the device gives — the Passphrase
/// does not open the Library, most often — and the window stays for another
/// try. Success hides the window; the explorer hears of it in its next answer
/// about what the server is doing.
#[tauri::command]
pub async fn unlock_library(
    app: AppHandle,
    shell: State<'_, Shell>,
    passphrase: String,
) -> Result<(), String> {
    // Before anything else, for the reason the open does it first (spec: DK-7).
    let passphrase = Passphrase::from_bytes(passphrase.into_bytes());

    let _turn = shell.opening.lock().await;
    let served = shell
        .served()
        .cloned()
        .ok_or("no Library is open in this app to unlock")?;
    if served.holds_library() {
        // A press that waited behind the unlock that landed.
        hide_window(&app);
        return Ok(());
    }

    let name = served.name.clone();
    let reopened = shell
        .runtime
        .spawn(async move { reopen(&name, move || Ok(passphrase)).await })
        .await;
    let library = match reopened {
        Ok(Ok(library)) => library,
        Ok(Err(refused)) => {
            tracing::info!("the Library could not be unlocked from the Passphrase window");
            return Err(format!("{:#}", anyhow::Error::new(refused)));
        }
        Err(panicked) => {
            tracing::error!(error = %panicked, "unlocking the Library stopped without an answer");
            return Err("unlocking the Library stopped without an answer".to_owned());
        }
    };
    match served.unlock(library) {
        Ok(Unlocked::Now) => {
            tracing::info!(operation = "unlock", "the Library was unlocked in place")
        }
        Ok(Unlocked::Already) => {}
        Err(refused) => {
            tracing::warn!("the Library the Passphrase reopened is not the one being served");
            return Err(format!("{refused:#}"));
        }
    }
    hide_window(&app);
    Ok(())
}
