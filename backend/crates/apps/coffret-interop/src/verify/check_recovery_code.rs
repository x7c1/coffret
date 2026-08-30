use anyhow::{bail, Context, Result};
use coffret_format::RecoveryCode;

use crate::fixture_set::FixtureReader;
use crate::hex;
use crate::manifest::RecoveryCodeFixture;

use super::same;

pub(super) fn check_recovery_code(
    reader: &FixtureReader,
    fixture: &RecoveryCodeFixture,
) -> Result<()> {
    let bytes = reader.read(&fixture.file)?;
    let Ok(text) = std::str::from_utf8(&bytes) else {
        bail!("a Recovery Code is text, and these bytes are not UTF-8");
    };

    let code = RecoveryCode::parse(text).context("reading the code the other side wrote")?;
    same(
        "master_key",
        &hex::encode(code.master_key().as_bytes()),
        &hex::encode(fixture.master_key()?.as_bytes()),
    )?;
    same("epoch", &code.epoch(), &fixture.epoch()?)
}
