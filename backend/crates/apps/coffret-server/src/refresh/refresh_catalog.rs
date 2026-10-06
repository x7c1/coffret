use coffret_device::CatchUpOutcome;

use crate::api_error::ApiError;
use crate::state::ServerState;

use super::run;

/// Catches the catalog up because somebody asked what is new.
///
/// [`catch_up`](coffret_device::OpenLibrary::catch_up) and nothing around it: the
/// Journal records this device has not seen are replayed and the run stops there
/// (spec: CK-9).
///
/// The outcome is this function's own value and goes back to whoever asked,
/// unlike a fill's or a sync's: a refresh is over when the replay is, and there
/// is no progress to publish along the way.
pub async fn refresh_catalog(state: &ServerState) -> Result<CatchUpOutcome, ApiError> {
    run::catch_up(state, "refresh").await
}

/// Catches the catalog up because the grant Storage had stopped taking was
/// renewed.
///
/// The same catch-up under the operation that asked for it: what the grant
/// running out refused was most likely a catch-up, and the listing a person was
/// looking at when they reconnected is only current once one lands.
pub(crate) async fn catch_up_after_reconnect(
    state: &ServerState,
) -> Result<CatchUpOutcome, ApiError> {
    run::catch_up(state, "reconnect").await
}
