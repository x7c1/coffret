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
    /// as `error`, and it is one of `bad_path` or `bad_request` (400),
    /// `unauthorized` (403), `no_such_entry` (404), `declined` (409), `storage`
    /// or `unverified` (502), and `server` (500).
    ///
    /// The whole set is named here because a browser writes a branch per kind,
    /// and a kind it has never heard of is one it falls off the end of. Adding
    /// one is adding a case to every caller.
    kind: &'static str,
    /// One sentence a person could read, written here rather than borrowed.
    message: String,
    /// Which way something was declined, where it was: `unmapped`,
    /// `unmaterializable`, `surfaced`, or `locked` for a fetch (spec: EP-11),
    /// and `pack_resident` for a file that would replace an Entry inside a Pack
    /// (spec: PK-10, PK-12). Present exactly where the kind is `declined`, and
    /// the whole set for the same reason.
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

    /// The request is not one this server answers, whoever sent it.
    ///
    /// The one refusal made before a route is reached, so it is about the
    /// caller and never about the Library: nothing in it says whether the path
    /// exists, whether the folder is mapped, or whether anything at all was
    /// asked for. `403` rather than `401`, because there is no challenge to
    /// answer here — the key is read off this device's disk, and a caller that
    /// cannot read it has nothing to try again with.
    pub(crate) fn unauthorized(message: &'static str) -> Self {
        Self::plain(StatusCode::FORBIDDEN, "unauthorized", message.to_owned())
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

    /// Nowhere on this device stands for the part of the Library that was named
    /// (spec: EP-9).
    ///
    /// The same verdict a fetch under an unmapped folder arrives at, said before
    /// anything is attempted rather than after: a drop onto a folder this device
    /// has no folder for has nowhere to put a single one of its files, so the
    /// whole of it is refused at once instead of once per file.
    pub fn no_folder_here() -> Self {
        Self {
            status: StatusCode::CONFLICT,
            kind: "declined",
            message: "no folder on this device holds this part of the Library".to_owned(),
            reason: Some("unmapped"),
            surfaced: None,
            cause: None,
        }
    }

    /// A file would replace an Entry whose Container is a Pack (spec: PK-15).
    ///
    /// Carrying such a change in is read-modify-replace over the whole Pack,
    /// which coffret does not do yet (spec: PK-10, PK-11, PK-12) — so a sync
    /// would find the changed file, surface it, and leave the Pack byte for byte
    /// as it is. Writing the file anyway would leave it sitting in a mapped
    /// folder that nothing can ever carry into the Library, which is the one
    /// state a person must not be put in silently. The refusal is made before any
    /// byte is written, and it names the file it is about.
    pub fn pack_resident() -> Self {
        Self {
            status: StatusCode::CONFLICT,
            kind: "declined",
            message: "the Library holds this file inside a Pack, and coffret cannot replace one \
                      of those yet"
                .to_owned(),
            reason: Some("pack_resident"),
            surfaced: None,
            cause: None,
        }
    }

    /// The request itself is not one this route can read.
    ///
    /// Kept apart from [`bad_path`](Self::bad_path), which is about a path a
    /// caller named: this is the envelope around it — a multipart body that ends
    /// mid-part, a boundary that is not one. There is nothing about the Library
    /// in it, and nothing for a screen to say beyond that the request did not
    /// arrive whole.
    pub fn bad_request(cause: impl error::Error + Send + Sync + 'static) -> Self {
        Self::plain(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "the request did not arrive as something this route can read".to_owned(),
        )
        .caused_by(cause)
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

    /// Which kind of refusal this is.
    ///
    /// These four are for the one caller that has a refusal and no response to
    /// put it in: the background fill, which reports what it found in an
    /// activity rather than by answering a request. They are the four fields a
    /// refusal goes out with and no more — what a refusal never says on the
    /// wire is what the layer below reported, and that stays unreachable from
    /// here as it is unreachable from a body.
    pub(crate) fn kind(&self) -> &'static str {
        self.kind
    }

    /// Which way a fetch was declined, where it was.
    pub(crate) fn reason(&self) -> Option<&'static str> {
        self.reason
    }

    /// The finding the fetch reported.
    pub(crate) fn surfaced(&self) -> Option<&'static str> {
        self.surfaced
    }

    /// The one sentence a person could read.
    pub(crate) fn message(&self) -> &str {
        self.message.as_str()
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
