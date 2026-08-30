use anyhow::{Context, Result};
use coffret_format::{generate_master_key, RecoveryCode};
use coffret_model::{MasterKey, MasterKeyEpoch};

use crate::fixture_set::{FixtureWriter, BLOBS_DIR};
use crate::hex;
use crate::manifest::RecoveryCodeFixture;

use super::{EPOCH, LATE_EPOCH};

/// Writes the Recovery Codes of the set: one bare, one as it would be printed.
///
/// The second carries a key of its own and an epoch past what 32 bits hold, so
/// a reader that took the epoch for four bytes, or that carried the set's one
/// Master Key into every code, disagrees here rather than passing.
pub(super) fn write_recovery_codes(
    writer: &FixtureWriter,
    master_key: &MasterKey,
) -> Result<Vec<RecoveryCodeFixture>> {
    let bare = RecoveryCode::encode(master_key, MasterKeyEpoch::new(EPOCH)?);
    let other = generate_master_key().context("drawing the second Recovery Code's Master Key")?;
    let grouped = RecoveryCode::encode(&other, MasterKeyEpoch::new(LATE_EPOCH)?);

    Ok(vec![
        write_one(writer, "recovery-code", master_key, EPOCH, bare.as_str())?,
        write_one(
            writer,
            "recovery-code-grouped",
            &other,
            LATE_EPOCH,
            &grouped.to_grouped_string(),
        )?,
    ])
}

fn write_one(
    writer: &FixtureWriter,
    fixture: &str,
    master_key: &MasterKey,
    epoch: u64,
    code: &str,
) -> Result<RecoveryCodeFixture> {
    let file = writer.write(BLOBS_DIR, &format!("{fixture}.txt"), code.as_bytes())?;
    Ok(RecoveryCodeFixture {
        fixture: fixture.to_owned(),
        file,
        master_key: hex::encode(master_key.as_bytes()),
        epoch,
    })
}
