use serde::Deserialize;

/// What `files.generateIds` answers with.
#[derive(Debug, Deserialize)]
pub struct GeneratedIds {
    /// The identifiers minted, as many as were asked for.
    pub ids: Vec<String>,
}
