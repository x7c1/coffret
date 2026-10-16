use tracing::debug;

/// Where one of the things an Entry fetch needs before its parcels came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Origin {
    /// This run read it from Storage.
    Read,
    /// The [`FetchMemory`](super::FetchMemory) kept it from an earlier read.
    Kept,
    /// Another run was reading it, and this one waited for that read.
    Joined,
}

impl Origin {
    /// How many reads of Storage this one stands for.
    pub(super) fn reads(self) -> u64 {
        match self {
            Self::Read => 1,
            Self::Kept | Self::Joined => 0,
        }
    }
}

/// What one Entry fetch read from Storage, by kind: the round trips one page
/// open costs, read off the log.
///
/// A front is two requests — the header and the meta section behind it
/// (spec: FM-2) — and is counted once here, as one front read.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct StorageReads {
    listings: u64,
    key_tables: u64,
    fronts: u64,
    parcels: u64,
}

impl StorageReads {
    pub(super) fn listing(&mut self, origin: Origin) {
        self.listings += origin.reads();
    }

    pub(super) fn key_table(&mut self, origin: Origin) {
        self.key_tables += origin.reads();
    }

    pub(super) fn front(&mut self, origin: Origin) {
        self.fronts += origin.reads();
    }

    pub(super) fn parcels(&mut self, read: u64) {
        self.parcels += read;
    }

    /// Says them, at debug level: what a person timing a page open reads to
    /// tell a fetch that waited on a read ahead from one that read for itself.
    pub(super) fn log(&self, verdict: &'static str) {
        debug!(
            verdict,
            listings = self.listings,
            key_tables = self.key_tables,
            fronts = self.fronts,
            parcels = self.parcels,
            "what a partial fetch read from Storage",
        );
    }
}
