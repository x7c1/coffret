//! What the folders of the Library come to, as the [`Index`] answers it.
//!
//! A Library stores no folders: an Entry Path is one string and `/` is the only
//! logical separator in it (spec: EP-2), so a folder is what the separators
//! imply. These are the answers a reader of folders asks of the Index — one
//! folder's children with what each comes to, every folder there is, and what
//! the whole Library comes to — and the derivation of each out of the
//! subtree reads the port has always had, which is what the port's own
//! defaults answer with and what an adapter keeping the figures apart has to
//! agree with.

use std::collections::BTreeSet;

use coffret_model::EntryPath;

use crate::index::Index;
use crate::index_error::IndexResult;

mod folder_summary;
pub use folder_summary::FolderSummary;

mod folder_file;
pub use folder_file::FolderFile;

mod folder_contents;
pub use folder_contents::FolderContents;

mod library_summary;
pub use library_summary::LibrarySummary;

mod derive_folder_contents;
pub use derive_folder_contents::derive_folder_contents;

mod derive_library_summary;
pub use derive_library_summary::derive_library_summary;

mod derive_folders;
pub use derive_folders::derive_folders;

/// The paths under `folder` whose Entry this device has (spec: EP-10).
async fn present_paths(
    index: &(impl Index + ?Sized),
    folder: Option<&EntryPath>,
) -> IndexResult<BTreeSet<EntryPath>> {
    Ok(index
        .present_under(folder)
        .await?
        .into_iter()
        .map(|local| local.observation.path)
        .collect())
}
