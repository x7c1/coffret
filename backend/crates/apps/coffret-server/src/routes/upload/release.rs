use coffret_device::{OpenLibrary, Redacted};
use tracing::{info, warn};

use super::drop_reservation::DropReservation;

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
    reservation: Option<DropReservation>,
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

/// What becomes of a drop's reservation when the drop is stopped part way
/// (spec: PK-17).
///
/// One the drop made is released, as for any drop that armed nothing: what it
/// left in the folder waits there as anything copied into a mapped folder
/// does. One it took over from a drop cut off earlier is kept as the drop in
/// progress it was, with whatever this drop added to it: releasing it would
/// leave the files the earlier drop received loose in the folder, for a sync
/// to carry in one Container each, where dropping the folder once more still
/// brings the whole book in as one Pack.
pub(super) async fn release_stopped(
    library: &OpenLibrary,
    reservation: Option<DropReservation>,
    operation: &'static str,
) {
    match reservation {
        Some(held) if held.taken_over => info!(
            operation,
            "a drop that took over one cut off was stopped; it stays in progress for the \
             next drop of its folder",
        ),
        other => release(library, other, operation).await,
    }
}
