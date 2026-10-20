use std::ffi::OsStr;

use anyhow::Context;

/// The variable the server's own binary reads its idle interval from
/// (spec: DK-4), which a desktop shell reads the same way so that a machine
/// sets it once for both.
pub const IDLE_MINUTES: &str = "COFFRET_IDLE_MINUTES";

/// The interval when nothing sets one, the server binary's own default.
const DEFAULT_IDLE_MINUTES: u64 = 30;

/// How many minutes in which nothing is read from or written to the Library
/// pass before the server locks it, from what [`IDLE_MINUTES`] was set to:
/// `None`, or the empty string, where it was not set.
///
/// Refused rather than defaulted where it is set and is not a whole number of
/// at least one: somebody who set it meant something by it, and a server that
/// quietly used another interval would be the one they did not ask for.
pub fn idle_minutes(setting: Option<&OsStr>) -> anyhow::Result<u64> {
    let Some(value) = setting.filter(|value| !value.is_empty()) else {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn read(text: &str) -> anyhow::Result<u64> {
        idle_minutes(Some(OsStr::new(text)))
    }

    #[test]
    fn an_unset_interval_is_the_servers_default() {
        assert_eq!(idle_minutes(None).unwrap(), DEFAULT_IDLE_MINUTES);
        assert_eq!(read("").unwrap(), DEFAULT_IDLE_MINUTES);
    }

    #[test]
    fn a_whole_number_of_at_least_one_is_taken() {
        assert_eq!(read("1").unwrap(), 1);
        assert_eq!(read("90").unwrap(), 90);
    }

    #[test]
    fn anything_else_is_refused_and_said() {
        for refused in ["0", "-1", "1.5", "thirty", " 5"] {
            let error = read(refused).expect_err(refused);
            assert!(error.to_string().contains(IDLE_MINUTES), "{error}");
        }
    }
}
