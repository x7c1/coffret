use std::collections::BTreeSet;
use std::ffi::OsString;

use tracing::info;

use crate::fetch::fetch_error::FetchResult;
use crate::fetch::kept_parcels::KeptParcels;
use crate::index::Index;

/// Discards every parcel file in the parcel directory that no held-parcel row
/// names, and says how many it discarded (spec: PK-21).
///
/// A fetch writes a parcel's row before its file (spec: OC-2), and a letting go
/// that runs beside it — a catch-up's, which takes none of the turns a fetch
/// holds — can read the fresh row, find no file yet, and let the parcel go before
/// the file is written. That file is then ciphertext nothing names, which no
/// letting go would ever reach, up to a parcel's length each. This is what
/// reaches it.
///
/// Only where no fetch can be running: a fetch between writing its row and its
/// file is exactly a row this would read before the file it lists. So it is
/// run as a server opens the Library, after the catch-up's own letting go and
/// before anything is served. Only names a kept parcel could have are
/// discarded; the directory is this device's own, but whatever else somebody
/// put in it is not this to remove.
pub async fn sweep_unnamed_parcels(
    index: &dyn Index,
    kept: &KeptParcels<'_>,
) -> FetchResult<usize> {
    let named: BTreeSet<OsString> = index
        .held_parcels()
        .await?
        .into_iter()
        .filter_map(|parcel| parcel.path.file_name().map(ToOwned::to_owned))
        .collect();
    let mut discarded = 0;
    for name in kept.files.list(kept.dir).await? {
        if named.contains(&name) || !is_parcel_name(&name) {
            continue;
        }
        kept.files.discard(&kept.dir.join(&name)).await?;
        discarded += 1;
    }
    if discarded > 0 {
        info!(
            operation = "sweep_parcels",
            parcels = discarded,
            "discarded the parcel files no row named",
        );
    }
    Ok(discarded)
}

/// Whether `name` is one [`KeptParcels`] could have given a parcel file.
fn is_parcel_name(name: &OsString) -> bool {
    name.to_str().is_some_and(|name| {
        name.strip_suffix(".parcel")
            .and_then(|stem| stem.rsplit_once('-'))
            .is_some_and(|(container, index)| !container.is_empty() && index.parse::<u64>().is_ok())
    })
}
