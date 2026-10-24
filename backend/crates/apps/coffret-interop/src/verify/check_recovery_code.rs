use coffret_format::RecoveryCode;

use crate::fixture_set::FixtureReader;
use crate::manifest::RecoveryCodeFixture;
use crate::{hex, Error, Result, Step};

use super::same;

pub(super) fn check_recovery_code(
    reader: &FixtureReader,
    fixture: &RecoveryCodeFixture,
) -> Result<()> {
    let bytes = reader.read(&fixture.file)?;
    let text = std::str::from_utf8(&bytes).map_err(|cause| Error::RecoveryCodeNotText { cause })?;

    let code = RecoveryCode::parse(text).map_err(Error::format(Step::ParsingRecoveryCode))?;
    same(
        "master_key",
        &hex::encode(code.master_key().as_bytes()),
        &hex::encode(fixture.master_key()?.as_bytes()),
    )?;
    same("epoch", &code.epoch(), &fixture.epoch()?)
}
