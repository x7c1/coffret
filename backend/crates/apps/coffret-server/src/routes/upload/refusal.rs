use crate::api_error::ApiError;

/// What a part was not taken for, and how far that reaches.
pub(super) enum Refusal {
    /// About this file alone, and the drop goes on: one file the Library will
    /// not have is not the file beside it.
    Part(ApiError),
    /// About the request, which stops here. What has outrun a budget or the room
    /// on this device is no truer of the next part than of this one, so there is
    /// nothing to be gained by reading the rest.
    Request(ApiError),
}

/// Anything that goes wrong about one file is about that one file, so `?` on it
/// says so and the drop carries on.
///
/// Which is what keeps the other variant honest: a refusal that stops the whole
/// request is spelled out at each of the few places that mean it, and nothing
/// becomes one by falling through a conversion.
impl From<ApiError> for Refusal {
    fn from(refusal: ApiError) -> Self {
        Self::Part(refusal)
    }
}

impl From<coffret_device::Error> for Refusal {
    fn from(cause: coffret_device::Error) -> Self {
        Self::Part(cause.into())
    }
}
