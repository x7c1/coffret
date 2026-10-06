//! The one setting the shell reads for itself.

use anyhow::Context;

/// The variable the server's own binary reads its idle interval from
/// (spec: DK-4), read here the same way so that a machine sets it once for
/// both.
const IDLE_MINUTES: &str = "COFFRET_IDLE_MINUTES";

/// The interval when nothing sets one, the server binary's own default.
const DEFAULT_IDLE_MINUTES: u64 = 30;

/// How many minutes in which nothing is read from or written to the Library
/// pass before the server locks it.
///
/// Refused rather than defaulted where it is set and is not a whole number of
/// at least one: somebody who set it meant something by it, and a server that
/// quietly used another interval would be the one they did not ask for.
pub fn idle_minutes() -> anyhow::Result<u64> {
    let Some(value) = std::env::var_os(IDLE_MINUTES).filter(|value| !value.is_empty()) else {
        return Ok(DEFAULT_IDLE_MINUTES);
    };
    let text = value
        .to_str()
        .with_context(|| format!("{IDLE_MINUTES} is not text"))?;
    let minutes: u64 = text
        .parse()
        .with_context(|| format!("{IDLE_MINUTES} is not a whole number of minutes: {text:?}"))?;
    anyhow::ensure!(
        minutes >= 1,
        "{IDLE_MINUTES} must be at least 1, not {minutes}"
    );
    Ok(minutes)
}
