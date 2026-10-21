use anyhow::Context;
use coffret_device::{walk_drop, Dropped};

use super::Desktop;

impl Desktop {
    /// Walks what a drop onto the explorer's window let go of, given as the
    /// whole path of each item, and holds what it took for the import the
    /// explorer then makes (spec: LA-26).
    ///
    /// What comes back is what the page is told: a one-time handle and, per
    /// file, its path relative to the folder it was dropped from, its size and
    /// its modification time — never a path on this device. Symbolic links are
    /// neither followed nor taken, and whatever was not taken is said beside
    /// what was (spec: LA-29).
    ///
    /// Run on the core's runtime and waited for, from a thread that is not one
    /// of its own: the shell calls it off its main thread.
    pub fn take_drop(&self, paths: &[String]) -> anyhow::Result<Dropped> {
        let opened = self
            .shared
            .served()
            .context("no Library is open in this app")?;
        let walk = self.runtime.block_on(walk_drop(paths));
        opened
            .state
            .drops
            .hold(walk)
            .context("a drop handle could not be drawn")
    }
}
