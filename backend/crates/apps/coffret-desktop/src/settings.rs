//! The one setting the app reads for itself.

use coffret_desktop_core::IDLE_MINUTES;

/// How many minutes in which nothing is read from or written to the Library
/// pass before the server locks it: the variable the server's own binary reads
/// its idle interval from (spec: DK-4), read here the same way so that a
/// machine sets it once for both, and refused where it is set to something that
/// is not an interval ([`coffret_desktop_core::idle_minutes`]).
pub fn idle_minutes() -> anyhow::Result<u64> {
    coffret_desktop_core::idle_minutes(std::env::var_os(IDLE_MINUTES).as_deref())
}
