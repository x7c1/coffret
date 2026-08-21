use super::LogSettings;

/// How many bytes of logs are kept before the oldest are dropped.
pub(super) const DEFAULT_MAX_TOTAL_BYTES: u64 = 8 * 1024 * 1024;

/// How large one file grows before the next one is started.
///
/// A fraction of the ceiling rather than all of it: pruning can only drop whole
/// files, so several smaller ones lose less evidence than one large one.
pub(super) const DEFAULT_MAX_FILE_BYTES: u64 = 1024 * 1024;

/// The smallest a file is allowed to be.
///
/// Below this a single formatted event would not fit, and every write would
/// start a file of its own.
const MIN_FILE_BYTES: u64 = 1024;

impl LogSettings {
    /// The ceiling and the per-file size, made consistent with each other.
    ///
    /// A file may never be larger than the whole budget, or pruning could not
    /// bring the total back under it — the file being written is never a
    /// candidate to drop. A floor applies as well, so that a mistaken ceiling
    /// of a few bytes still leaves room for one event.
    pub(crate) fn sizes(&self) -> (u64, u64) {
        let total = self.max_total_bytes.max(MIN_FILE_BYTES);
        let per_file = self.max_file_bytes.clamp(MIN_FILE_BYTES, total);
        (total, per_file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_may_not_be_larger_than_the_whole_budget() {
        let settings = LogSettings::new("/tmp/logs")
            .with_ceiling(4096)
            .with_max_file_bytes(1024 * 1024);

        assert_eq!(settings.sizes(), (4096, 4096));
    }

    #[test]
    fn a_ceiling_too_small_to_hold_an_event_is_raised_to_one() {
        let settings = LogSettings::new("/tmp/logs").with_ceiling(1);
        let (total, per_file) = settings.sizes();

        assert_eq!(total, MIN_FILE_BYTES);
        assert_eq!(per_file, MIN_FILE_BYTES);
    }
}
