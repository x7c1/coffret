use coffret_usecase::device_state::Mapping;
use coffret_usecase::IndexError;

/// What listing this device's mappings found.
///
/// The ordinary answer is the catalog's own record. The other one exists
/// because the mappings are the one piece of this device's state a refused
/// Index file still gives up on request — its two columns stay readable in
/// every layout, so a Library whose catalog this build cannot open is not a
/// dead end for recovering them. A caller told which of the two happened can
/// say so, and show the mappings either way.
#[derive(Debug)]
pub enum MappingListing {
    /// What the catalog itself recorded.
    Recorded(Vec<Mapping>),
    /// What was read straight out of a file whose catalog `open` refused, kept
    /// with the refusal that made reading it this way necessary.
    FromRefusedFile {
        /// The mappings read out of the refused file.
        mappings: Vec<Mapping>,
        /// Why the catalog itself would not open.
        refusal: IndexError,
    },
}

impl MappingListing {
    /// The mappings, however they were found, the Library root first.
    pub fn mappings(&self) -> &[Mapping] {
        match self {
            Self::Recorded(mappings) => mappings,
            Self::FromRefusedFile { mappings, .. } => mappings,
        }
    }
}
