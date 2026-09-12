use coffret_device::Error;

use crate::api_error::ApiError;

/// What a part was not taken for, and how far that reaches.
pub(super) enum Refusal {
    /// About this file alone, and the drop goes on: one file the Library will
    /// not have is not the file beside it.
    Part(ApiError),
    /// About the request, which stops here: whatever made it is no truer of the
    /// next part than of this one, so there is nothing to be gained by reading
    /// the rest. A budget this request has outrun, room this device has not for
    /// what is still coming or could not be asked about at all, a stream that
    /// broke before the next part, the state of a mapped root the parts are
    /// going through — one condition gathers them, and it is what the next of
    /// them is judged by.
    ///
    /// That condition is what the variant is chosen by, rather than the wire
    /// kind the refusal goes out under: a refused root reaches a page as
    /// `declined` the way a declined placement does (spec: EP-13), and is
    /// nevertheless about the whole drop.
    Request(ApiError),
}

/// Anything that goes wrong about one file is about that one file, so `?` on it
/// says so and the drop carries on.
///
/// Which is what keeps the other variant honest: an [`ApiError`]'s kinds are
/// the wire's, and they do not divide by reach — `declined` is what a refused
/// root and a refused part both go out under — so nothing in one tells this type
/// how far it reaches. This is therefore the conversion that can never make a
/// refusal about the request, and every place that decides the reach from the
/// request itself — a budget it has outrun, the room this device has, a stream
/// that broke — writes [`Request`](Self::Request) out in full.
///
/// The conversion below is the one exception, and it is one because it has a
/// failure kind rather than a wire kind to read: a device error whose kind
/// settles the reach wherever it is met. So `?` on one of those may stop the
/// request, and that arm is the whole of where that is decided.
impl From<ApiError> for Refusal {
    fn from(refusal: ApiError) -> Self {
        Self::Part(refusal)
    }
}

impl From<Error> for Refusal {
    fn from(cause: Error) -> Self {
        match cause {
            // The root the parts of this drop go into: in a drop onto a folder
            // it is one root for all of them, and at the Library root, where
            // the parts carry mappings of their own (spec: EP-9), it is the
            // first of those to be refused. Either way nothing is placed into
            // it and the request fails as a whole, the way a declined placement
            // fails a single writer's (spec: EP-11, EP-13).
            refused @ Error::RootRefused { .. } => Self::Request(refused.into()),
            other => Self::Part(other.into()),
        }
    }
}
