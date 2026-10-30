use coffret_device::MovePreview;

use crate::api_error::ApiError;

/// Every refusal `preview` says the move would meet, in the order a person is
/// told them: the rename's own, then this device's edited files, then what is
/// still on its way into the Library. Empty where the move may go ahead.
///
/// The same refusals [`move_pairs`](super::move_pairs) stops with, which is
/// why a preview and a move cannot disagree about them except where the
/// Library or the disk changed in between.
pub fn refusals_of(preview: &MovePreview) -> Vec<ApiError> {
    let mut refusals = Vec::new();
    if let Some(refusal) = &preview.rename.refused {
        refusals.push(ApiError::move_refused_by(refusal));
    }
    if !preview.rename.edited.is_empty() {
        refusals.push(ApiError::move_of_edited(&preview.rename.edited));
    }
    if !preview.uploading.is_empty() {
        refusals.push(ApiError::move_of_uploading(&preview.uploading));
    }
    refusals
}
