use coffret_format::StoredMasterKey;

use crate::fixture_set::{FixtureWriter, BLOBS_DIR};
use crate::manifest::{RefusedStoredMasterKeyFixture, StoredMasterKeyRefusal};
use crate::Result;

/// Where the iterations field lies in the fixed part of the form (spec: KD-9).
const ITERATIONS: std::ops::Range<usize> = 12..16;

/// Writes a form with several faults: one byte past the length its recorded
/// salt length makes it, and iterations at `u32::MAX`.
///
/// A reader checks the total length before the cost (spec: KD-9), so the
/// refusal is the length's. Made by editing a form this side wrote, so every
/// other field is one a reader accepts.
pub(super) fn write_refused_stored_master_key(
    writer: &FixtureWriter,
    stored: &StoredMasterKey,
) -> Result<RefusedStoredMasterKeyFixture> {
    let mut bytes = stored.as_bytes().to_vec();
    bytes[ITERATIONS].copy_from_slice(&u32::MAX.to_be_bytes());
    bytes.push(0);

    let file = writer.write(BLOBS_DIR, "stored-master-key-several-faults.bin", &bytes)?;
    Ok(RefusedStoredMasterKeyFixture {
        fixture: "stored-master-key-several-faults".to_owned(),
        file,
        refusal: StoredMasterKeyRefusal::StoredMasterKeyLengthMismatch,
    })
}
