use coffret_device::{OpenLibrary, Redacted, Unrenewed};
use tracing::error;

use super::{Consent, ConsentFlow};

/// The flow that ships: Google's consent page, the loopback redirect, and the
/// account's cache (spec: SA-1, SA-2, SA-4, SA-6).
pub struct DriveConsent;

impl Consent for DriveConsent {
    fn ask(
        &self,
        library: &OpenLibrary,
        show: Box<dyn FnOnce(&str) + Send>,
    ) -> Option<ConsentFlow> {
        let grant = library.grant.clone()?;
        Some(Box::pin(async move {
            grant.renew(show).await.map_err(|failure| {
                // The whole chain into the log, redacted, which is the one place
                // a flow that failed for any reason other than the person says
                // why: the explorer is told only which of three endings it was.
                error!(
                    operation = "reconnect",
                    error = failure.redacted().as_str(),
                    "the grant was not renewed",
                );
                Unrenewed::of(&failure)
            })
        }))
    }
}
