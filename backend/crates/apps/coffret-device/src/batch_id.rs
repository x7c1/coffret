//! What this device calls the batch one run produces.
//!
//! A [`BatchId`] never leaves the device — no Journal record or Snapshot carries
//! one — so any spelling a device can keep unique among its own unfinished
//! batches will do (spec: OC-2). This is that spelling: the moment the run
//! started, and four random bytes.
//!
//! The moment is there for a person rather than for the machine. A batch id
//! names a directory of spool files that an interrupted run leaves behind, and
//! whoever comes to look at one wants to know when it was made without opening
//! anything. The random bytes are what actually make it unique: two runs started
//! in the same second are the ordinary case on a device that scripts its syncs.

use std::time::{SystemTime, UNIX_EPOCH};

use coffret_usecase::device_state::{BatchId, DeviceTime};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// How many random bytes go in a batch id.
const RANDOM_BYTES: usize = 4;

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

/// A name for this run's batch that no unfinished batch of this device holds.
///
/// A clock that cannot be turned into a date leaves the timestamp at the Unix
/// epoch rather than failing the run: the id is opaque, the random half is what
/// keeps it unique, and no batch is worth refusing over how its name reads.
pub(crate) fn next_batch_id(now: DeviceTime) -> BatchId {
    let stamp = OffsetDateTime::from_unix_timestamp(now.as_unix_seconds())
        .unwrap_or(OffsetDateTime::UNIX_EPOCH)
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned());

    let mut bytes = [0_u8; RANDOM_BYTES];
    // A batch id is not key material and nothing rests on its unpredictability,
    // so an entropy source that refuses is not a reason to refuse the run: the
    // clock alone still separates it from every batch of another second.
    let _ = getrandom::fill(&mut bytes);
    let random: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();

    BatchId::new(format!("{stamp}-{random}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // The spelling is what makes a spool directory legible to whoever finds one:
    // the second the run started, then the eight characters that separate it
    // from another run of the same second.
    #[test]
    fn a_batch_is_named_after_the_second_it_started_in() {
        let id = next_batch_id(DeviceTime::from_unix_seconds(1_772_000_000));
        let name = id.as_str();

        let (stamp, random) = name
            .split_once("Z-")
            .unwrap_or_else(|| panic!("{name:?} must be a timestamp and a suffix"));
        assert_eq!(stamp, "2026-02-25T06:13:20");
        assert_eq!(random.len(), RANDOM_BYTES * 2);
        assert!(
            random
                .chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
            "the suffix is lowercase hex: {name:?}"
        );
    }

    // Two runs in the same second are the ordinary case on a device that scripts
    // its syncs, and a batch id that repeated would make one run's cleanup
    // reclaim the other's spools (spec: OC-2).
    #[test]
    fn two_batches_of_one_second_are_named_apart() {
        let now = DeviceTime::from_unix_seconds(1_772_000_000);
        assert_ne!(next_batch_id(now), next_batch_id(now));
    }

    // A clock before the Unix epoch is a clock, not a reason to refuse a run.
    #[test]
    fn a_clock_before_the_epoch_still_names_a_batch() {
        let id = next_batch_id(DeviceTime::from_unix_seconds(-1));
        assert!(
            id.as_str().starts_with("1969-12-31T23:59:59Z-"),
            "{id} must carry the moment the clock reported"
        );
    }
}
