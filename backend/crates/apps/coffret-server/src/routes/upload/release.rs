use coffret_device::{LiveReservation, OpenLibrary, Redacted};
use tracing::warn;

/// Releases a drop's reservation, where it made one, because nothing is armed
/// for the drop (spec: PK-17).
///
/// A release that fails is said and not raised: the drop has already been
/// answered for, and what the reservation left behind is a set of paths a
/// later freeze over them decides and releases like any other — until then a
/// sync leaves them alone and says they wait for their Pack, which is not
/// silent.
pub(super) async fn release(
    library: &OpenLibrary,
    reservation: Option<LiveReservation>,
    operation: &'static str,
) {
    let Some(held) = reservation else {
        return;
    };
    let id = held.id();
    if let Err(refused) = library.release_reservation(id).await {
        warn!(
            operation,
            reservation = %id,
            reason = %refused.redacted(),
            "the reservation of a drop that armed nothing could not be released",
        );
    }
}
