use crate::fetch::fetch_error::FetchResult;
use crate::index::Index;
use crate::parcel_files::ParcelFiles;

/// How many parcels this device keeps, and how many bytes their files take.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeptTally {
    /// The held parcels whose file is on the disk.
    pub count: u64,
    /// The length of those files together.
    pub bytes: u64,
}

/// Counts the parcels this device keeps, from the rows and a stat of the file
/// each names (spec: PK-21).
///
/// What a person is shown of a directory nothing else tells them is there: a
/// reader who opens the first page of many books leaves a parcel or two behind
/// for each, and over time that is gigabytes. The rows are a few dozen and a
/// stat is cheap, so this is asked as often as the answer it goes in is; the
/// directory is never walked. A row whose file is gone is not counted, because
/// nothing is kept for it — the next fetch meets it as a parcel not held.
pub async fn tally_kept_parcels(
    index: &dyn Index,
    files: &dyn ParcelFiles,
) -> FetchResult<KeptTally> {
    let mut tally = KeptTally::default();
    for parcel in index.held_parcels().await? {
        if let Some(len) = files.size(&parcel.path).await? {
            tally.count += 1;
            tally.bytes += len;
        }
    }
    Ok(tally)
}
