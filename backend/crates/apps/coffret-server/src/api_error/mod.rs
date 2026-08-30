//! The one shape every refusal on these routes takes.
//!
//! The value and the ways of naming one are here; what a failure from below
//! becomes is in [`from_error`], and what goes on the wire is in
//! [`into_response`].

use std::error;

use axum::http::StatusCode;
use coffret_device::{FetchError, Surfaced};

mod from_error;

mod into_response;

#[cfg(test)]
mod tests;

/// Everything that can come back instead of an answer, in one shape.
///
/// One shape and one place, because the browser is what reads these and a
/// browser branches on a status and a name rather than on prose. So each of
/// these carries a status the caller can act on — the path was not one, the
/// Library holds nothing there, the fetch was declined, Storage did not come
/// through — and a `kind` naming which of those it is.
///
/// What actually went wrong stays in [`cause`](Self::cause) and reaches the log,
/// never the body. Two reasons. A lower layer's message is written for whoever
/// is keeping the Library rather than for a page, and a body is exactly where a
/// message ends up being displayed verbatim; and some of those messages name an
/// Entry Path, which is the user's own name for their file (spec: EP-1) and not
/// something to be echoed back out of a failure.
pub struct ApiError {
    status: StatusCode,
    /// Which kind of refusal this is, for the caller to branch on. It travels
    /// as `error`, and it is one of `bad_path` (400), `no_such_entry` (404),
    /// `declined` (409), `storage` or `unverified` (502), and `server` (500).
    ///
    /// The whole set is named here because a browser writes a branch per kind,
    /// and a kind it has never heard of is one it falls off the end of. Adding
    /// one is adding a case to every caller.
    kind: &'static str,
    /// One sentence a person could read, written here rather than borrowed.
    message: String,
    /// Which way a fetch was declined, where it was (spec: EP-11): `unmapped`,
    /// `unmaterializable`, `surfaced`, or `locked`. Present exactly where the
    /// kind is `declined`, and the whole set for the same reason.
    reason: Option<&'static str>,
    /// The finding the fetch reported, by the name the device layer gives it:
    /// `ForeignFile`, `LocallyChanged`, `WitnessedDeletion`, or `KeyLost`.
    ///
    /// Present where the reason is `surfaced` or `locked`, and absent where it
    /// is `unmapped` or `unmaterializable` — those two are refusals no finding
    /// stands behind. The set is named here for the reason the others are: it is
    /// what a browser telling one declined path from another branches on.
    surfaced: Option<&'static str>,
    /// What the layer below reported. For the log, and for nothing else.
    cause: Option<Box<dyn error::Error + Send + Sync>>,
}

impl ApiError {
    /// The text a caller sent is not an Entry Path (spec: EP-2).
    pub fn bad_path(defect: &str) -> Self {
        Self::plain(
            StatusCode::BAD_REQUEST,
            "bad_path",
            format!("that is not an Entry Path: {defect}"),
        )
    }

    /// The Library holds no current Entry at the path (spec: EP-5).
    pub fn no_such_entry() -> Self {
        Self::plain(
            StatusCode::NOT_FOUND,
            "no_such_entry",
            "the Library holds nothing at that path".to_owned(),
        )
    }

    /// A fetch declined the path, and said why (spec: EP-11).
    ///
    /// A locked Container is its own reason rather than one finding among the
    /// others, because it is the one of them nothing about this device can
    /// resolve: the ciphertext is where it belongs and the key is gone
    /// (spec: KL-7, KL-17).
    pub fn declined(surfaced: &Surfaced) -> Self {
        let (reason, message) = match surfaced {
            Surfaced::KeyLost { .. } => (
                "locked",
                "the Library records no key for the Container holding this Entry",
            ),
            Surfaced::ForeignFile { .. } => (
                "surfaced",
                "a file this device did not put there stands where this Entry belongs",
            ),
            Surfaced::LocallyChanged { .. } => (
                "surfaced",
                "what this device wrote there has since changed or gone",
            ),
            Surfaced::WitnessedDeletion { .. } => (
                "surfaced",
                "this device witnessed the deletion of this Entry's file",
            ),
        };
        Self {
            status: StatusCode::CONFLICT,
            kind: "declined",
            message: message.to_owned(),
            reason: Some(reason),
            surfaced: Some(name_of(surfaced)),
            cause: None,
        }
    }

    /// A local file this device believed it had could not be read.
    pub fn unreadable(cause: std::io::Error) -> Self {
        Self::server(cause)
    }

    /// Something the server itself could not do, whatever it was.
    ///
    /// One constructor rather than one per site, because this is the one refusal
    /// whose body says nothing about what happened — the browser did nothing and
    /// can do nothing, and what actually went wrong travels as the cause to the
    /// log. Three spellings of that sentence would be three chances for one of
    /// them to start saying more.
    fn server(cause: impl error::Error + Send + Sync + 'static) -> Self {
        Self::plain(
            StatusCode::INTERNAL_SERVER_ERROR,
            "server",
            "the server could not answer".to_owned(),
        )
        .caused_by(cause)
    }

    fn plain(status: StatusCode, kind: &'static str, message: String) -> Self {
        Self {
            status,
            kind,
            message,
            reason: None,
            surfaced: None,
            cause: None,
        }
    }

    fn declined_as(reason: &'static str, message: &str, cause: FetchError) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            kind: "declined",
            message: message.to_owned(),
            reason: Some(reason),
            surfaced: None,
            cause: Some(Box::new(cause)),
        }
    }

    fn caused_by(mut self, cause: impl error::Error + Send + Sync + 'static) -> Self {
        self.cause = Some(Box::new(cause));
        self
    }
}

/// The name the device layer gives one finding (spec: EP-11).
fn name_of(surfaced: &Surfaced) -> &'static str {
    match surfaced {
        Surfaced::ForeignFile { .. } => "ForeignFile",
        Surfaced::LocallyChanged { .. } => "LocallyChanged",
        Surfaced::WitnessedDeletion { .. } => "WitnessedDeletion",
        Surfaced::KeyLost { .. } => "KeyLost",
    }
}
