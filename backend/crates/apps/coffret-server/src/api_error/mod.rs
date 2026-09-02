//! The one shape every refusal on these routes takes.
//!
//! The value and the ways of naming one are here; what a failure from below
//! becomes is in [`from_error`], and what goes on the wire is in
//! [`into_response`].

use std::error;

use axum::extract::multipart::MultipartError;
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
    /// `unauthorized` (403), `no_such_entry` (404), `declined` (409), `locked`
    /// (423), `storage` or `unverified` (502), and `server` (500).
    ///
    /// Two of them carry a second status, and neither is a second kind. A
    /// request that outran the server's resource envelope
    /// ([`Envelope`](crate::Envelope)) is `bad_request` at `413`, because what
    /// is wrong with it is its size rather than anything about the Library; and
    /// a device with no room left to take a drop is `server` at `507`, because
    /// it is this machine's state and nothing the browser did. A caller
    /// branching on the kind reads them as what they are and shows the
    /// sentence; one that wants the difference has the status.
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
    ///
    /// The `locked` here is a Container's and not this server's. It is one Entry
    /// whose Container the Library records no key for (spec: KL-7), which no
    /// Passphrase resolves; the server being locked is the `locked` *kind*
    /// above, which is the owner's own state and is resolved by the Passphrase.
    /// The two never appear together — a locked server declines nothing, because
    /// it fetches nothing.
    reason: Option<&'static str>,
    /// The finding the fetch reported, by the name the device layer gives it:
    /// `ForeignFile`, `LocallyChanged`, `WitnessedDeletion`, `UnreachablePlace`,
    /// or `KeyLost`.
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

    /// The server is locked, so nothing that needs the Master Key can be done
    /// (spec: DK-1, DK-2).
    ///
    /// Its own kind and not one of the admission fences' `unauthorized`, because
    /// they are opposite verdicts about opposite people. `unauthorized` is said
    /// to somebody who is not the owner of this Library and deliberately tells
    /// them nothing; this is said to the owner about their own device, and tells
    /// them everything — what state it is in, and the one thing that ends it.
    ///
    /// The sentence names the Passphrase because that is what DK-2 requires it
    /// to report, and it names starting the server again because that is the
    /// only place a Passphrase is typed.
    ///
    /// It also names both ways a server comes to be locked, because one of them
    /// is nobody's doing: whoever pressed the control knows what they pressed,
    /// but the person who left a book open and came back to turn a page never
    /// asked for anything and would otherwise read a locked server as a broken
    /// one. Which of the two it was is not tracked — the answer is the same
    /// either way, and the sentence says both rather than the state alone.
    ///
    /// `423` rather than `403`, for the reason the sentence is different: the
    /// request was perfectly legitimate and the resource is the thing that is
    /// shut, which is exactly what that status is for.
    pub(crate) fn locked() -> Self {
        Self::plain(
            StatusCode::LOCKED,
            "locked",
            "the Passphrase is required: this server is locked, either because it was asked to \
             be or because nothing had used it for a while, and it is unlocked by starting it \
             again with the Passphrase"
                .to_owned(),
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
            // The folder the descent stopped at stays out of the sentence, the
            // way every other local path does on these routes: it is named to
            // whoever is at a terminal keeping the Library, and this is one line
            // beside one row in a browser.
            Surfaced::UnreachablePlace { .. } => (
                "surfaced",
                "a folder on the way to this Entry is not a folder of this device's mapped \
                 folder",
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

    /// A multipart body that could not be read, or that outran the body limit
    /// the route is mounted with ([`Envelope`](crate::Envelope)).
    ///
    /// One constructor for both because both are the same thing said about one
    /// multipart body: the request did not arrive as one this route takes.
    /// Which of the two it was is the status, and the status is the extractor's
    /// own verdict rather than a second reading here — it is the half that knows
    /// whether it stopped because the boundary was wrong or because the bytes
    /// ran past what it was allowed to read.
    pub fn multipart(cause: MultipartError) -> Self {
        match cause.status() == StatusCode::PAYLOAD_TOO_LARGE {
            true => Self::too_large(
                "the drop as a whole is what passed that, rather than any one file in it — \
                 the same files in two drops are taken",
            )
            .caused_by(cause),
            false => Self::bad_request(cause),
        }
    }

    /// The request passed one of the budgets the server takes a drop within
    /// ([`Envelope`](crate::Envelope)).
    ///
    /// `413` and the `bad_request` kind: nothing about the Library is being
    /// refused here, and nothing about the request is wrong except its size. The
    /// sentence says which budget it was and what to do about it, because that
    /// is what whoever is at the browser can act on — and the three do not have
    /// one answer between them. Two of them are cleared by dropping the same
    /// files in two lots; the third is one file too large to be taken at all,
    /// and its sentence says so rather than leaving somebody to halve a drop
    /// that will be refused again.
    ///
    /// Where they get to read it, which is not certain. This is answered in the
    /// middle of a request that is still being sent, and a browser may report
    /// that as a transfer which failed rather than as an answer it was given. So
    /// whoever raises it says the same thing to the log, which is the half that
    /// arrives whatever the browser makes of the other.
    ///
    /// It stops the request where it stands. What had already landed is in the
    /// folder as the whole files they are — no part becomes visible before it is
    /// complete (spec: EP-11) — and nothing is armed for them. Nothing on this
    /// server arms one on its own either: they wait in the folder the way
    /// anything else copied into a mapped folder waits, until a later drop that
    /// lands something arms a sync or somebody asks for one.
    pub fn too_large(defect: &str) -> Self {
        Self::plain(
            StatusCode::PAYLOAD_TOO_LARGE,
            "bad_request",
            format!("that is more than this route takes: {defect}"),
        )
    }

    /// The volume this device's mapped folder is on has not the room for what is
    /// being sent.
    ///
    /// `507`, and the `server` kind, because it is a fact about this machine
    /// rather than about the request or the Library: the same drop onto the same
    /// folder would have been taken an hour ago. Said before the part it is about
    /// is written, so what it refuses is a disk being filled rather than a disk
    /// that already is.
    ///
    /// Neither number reaches the sentence. How much room a person's disk has is
    /// theirs, the browser can do nothing with it, and what they need to be told
    /// is which machine to go and look at and what to do there — the drop is
    /// made again once there is room, and nothing about it has to be undone
    /// first. They reach the log instead, where whoever went and looked is the
    /// one reading — and where this refusal would otherwise leave no account of
    /// itself at all, being the one `server` kind with no failure underneath it
    /// to record.
    pub fn no_room() -> Self {
        Self::plain(
            StatusCode::INSUFFICIENT_STORAGE,
            "server",
            "this device has not the room to take these files: the volume its folder for this \
             part of the Library is on is nearly full — free some room on it and drop them \
             again"
                .to_owned(),
        )
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
        Surfaced::UnreachablePlace { .. } => "UnreachablePlace",
        Surfaced::KeyLost { .. } => "KeyLost",
    }
}
