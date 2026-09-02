use serde::Serialize;

use super::refused_dto::RefusedDto;

/// What became of one drop.
///
/// Per part, because a drop is a handful of files and they are separate
/// questions: one name the Library holds inside a Pack does not stop the file
/// beside it landing. The two lists are the whole answer — what is now in the
/// folder, and what is not and why.
#[derive(Serialize)]
pub struct UploadDto {
    /// The Entry Paths the files were written at, in the order they arrived.
    ///
    /// Where they will stand in the Library once the flow the drop armed has
    /// carried them in. They are already in the folder, and the listing shows
    /// them from the next request onwards.
    pub(super) written: Vec<String>,
    /// The parts nothing was written for, each with the refusal it met.
    pub(super) refused: Vec<RefusedDto>,
}
