//! The refusals of a move of files and folders: the set the Library does not
//! admit, and the files a move would lose (spec: FM-15, EP-10).

use axum::http::StatusCode;
use coffret_device::{EntryPath, RenameRefusal};

use super::ApiError;

impl ApiError {
    /// A move refused before anything was written, which is
    /// `409 refused_move` whatever refused it.
    ///
    /// The one place that kind is built, for the reason
    /// [`refused_placement`](Self::refused_placement) is the one place its kind
    /// is: no `refused_move` goes out without its reason.
    fn refused_move(reason: &'static str, message: String) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            kind: "refused_move",
            message,
            reason: Some(reason),
            surfaced: None,
            cause: None,
            written: None,
        }
    }

    /// The rename the move is refused as: the set names nothing, two pairs
    /// overlap, or one pair is refused by the Library (spec: FM-15, EP-5).
    ///
    /// The reason is the refusal's own name for what is wrong, and the sentence
    /// is the rename's, which names the paths: they are the person's own, and
    /// the body is read by them (spec: EL-1 keeps them out of the log).
    pub(crate) fn move_refused_by(refusal: &RenameRefusal) -> Self {
        let reason = match refusal {
            RenameRefusal::Nothing | RenameRefusal::NothingAt { .. } => "nothing_to_move",
            RenameRefusal::Overlapping { .. } => "overlapping",
            RenameRefusal::SamePath => "same_path",
            RenameRefusal::ReservedNamespace { .. } => "reserved_namespace",
            RenameRefusal::IntoItself => "into_itself",
            RenameRefusal::DestinationTaken { .. } => "destination_taken",
            RenameRefusal::FileAndFolder { .. } => "file_and_folder",
            RenameRefusal::Rule(_) => "rename_rule",
        };
        Self::refused_move(reason, refusal.to_string())
    }

    /// Files of this device's in the move changed since it placed them
    /// (spec: EP-15). A file that cannot follow its Entry leaves this device,
    /// and the Library does not hold what changed.
    pub(crate) fn move_of_edited(edited: &[EntryPath]) -> Self {
        Self::refused_move(
            "edited",
            format!(
                "{} changed on this device since it was placed, and the Library does not hold \
                 what changed; carry it into the Library, or put it back, before moving it",
                named(edited, "this device's file", "of this device's files"),
            ),
        )
    }

    /// Files under the move this device has and the Library does not hold
    /// yet (spec: EP-10, PK-17).
    pub(crate) fn move_of_uploading(uploading: &[EntryPath]) -> Self {
        Self::refused_move(
            "uploading",
            format!(
                "{} not in the Library yet; wait until it is carried in before moving it",
                named(uploading, "the file", "of the files under it"),
            ),
        )
    }
}

/// The first of `paths` as a sentence names it, and how many others there are.
fn named(paths: &[EntryPath], one: &str, many: &str) -> String {
    match paths {
        [only] => format!("{one} \"{only}\" is"),
        [first, rest @ ..] => format!(
            "\"{first}\" and {} other{} {many} are",
            rest.len(),
            if rest.len() == 1 { "" } else { "s" }
        ),
        [] => format!("{one} is"),
    }
}
