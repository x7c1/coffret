use serde::Deserialize;

use crate::api::file_resource::FileResource;

/// What a listing answers with.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileList {
    /// The files on this page, absent from the answer when there are none.
    #[serde(default)]
    pub files: Vec<FileResource>,
    /// What to ask for the next page with, absent on the last one.
    pub next_page_token: Option<String>,
}
