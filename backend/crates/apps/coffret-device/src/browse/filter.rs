use coffret_model::DepositedAt;
use coffret_usecase::filter::{filter_library, FilterAsk, LibraryFilter, MOST_FILTER_ROWS};
use tracing::debug;

use crate::error::Result;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// The rows a tree filtered by `ask` keeps, out of the catalog alone
    /// (spec: LA-40, LA-41, LA-42).
    ///
    /// One read of every Entry, matched in memory: the Index holds no index of
    /// names, and a full scan of a hundred thousand Entries is about a tenth
    /// of a second. Nothing of Storage is asked.
    pub async fn filter(&self, ask: &FilterAsk) -> Result<LibraryFilter> {
        let entries = self.index.entries_under(None).await?;
        let scanned: Vec<(&str, DepositedAt)> = entries
            .iter()
            .map(|location| (location.path().as_str(), location.deposited_at))
            .collect();
        let filter = filter_library(&scanned, ask, MOST_FILTER_ROWS);
        debug!(
            operation = "filter",
            library = %self.library_id,
            entries = entries.len(),
            total = filter.total,
            truncated = filter.truncated,
            "filtered the whole Library",
        );
        Ok(filter)
    }
}
