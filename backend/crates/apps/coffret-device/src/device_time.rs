//! This device's clock, read once per run.

use std::time::{SystemTime, UNIX_EPOCH};

use coffret_usecase::device_state::DeviceTime;

/// This device's clock, as the flows write it down.
///
/// One reading per run, taken at the start: every observation a run records is
/// stamped with it, so one run's bookkeeping stands at one moment rather than at
/// as many moments as it touched files. Nothing about the Library's correctness
/// rests on it (spec: CP-7), which is why a clock that is before the Unix epoch
/// is recorded as it reads rather than refused.
pub(crate) fn now() -> DeviceTime {
    let seconds = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(since) => i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
        Err(before) => -i64::try_from(before.duration().as_secs()).unwrap_or(i64::MAX),
    };
    DeviceTime::from_unix_seconds(seconds)
}
