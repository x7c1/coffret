//! The one shape every refusal on these routes takes.
//!
//! The value, and the builders every way of naming one goes through, are here.
//! The ways of naming one are grouped by what they answer: [`admission`],
//! [`paths`], [`declines`], [`placements`] (with [`refused_root`]),
//! [`drop_budget`], [`local_folders`] and [`server`]. What a failure from below
//! becomes is in [`from_error`], what may be said about one in a diagnostic
//! event is in [`redact`], and what goes on the wire is in [`into_response`].

use axum::http::StatusCode;

mod admission;
pub(crate) use admission::WayBack;

mod declines;

mod drop_budget;

mod from_error;

mod into_response;

mod local_folders;

mod paths;

mod placements;

mod redact;

mod refused_root;
pub(crate) use refused_root::refused_root_said;

mod server;

#[cfg(test)]
mod tests;

// Every refusal as the wire carries it, written to the file the explorer's own
// cases read back through its types.
#[cfg(test)]
mod contract;
#[cfg(test)]
pub(crate) use contract::held_to;

/// The kind every refusal nobody outside this process can act on travels as.
///
/// Named here, where every other kind is spelled, because
/// [`Reported`](crate::Reported) mints two refusals outside this module — a run
/// whose worker ended without an answer travels as this, and a catch-up
/// abandoned at the startup deadline as [`STORAGE`] — and a browser branches on
/// their kinds exactly as it does on any other.
pub(crate) const SERVER: &str = "server";

/// The kind Storage not coming through travels as, named here for the reason
/// [`SERVER`] is.
pub(crate) const STORAGE: &str = "storage";

/// The reason a `storage` refusal carries where Storage no longer takes this
/// device's credential, which is the one Storage refusal a page offers a
/// gesture other than the retry for.
pub(crate) const UNAUTHENTICATED: &str = "unauthenticated";

/// What a placement under a folder this device has no folder for is told as
/// (spec: EP-9).
///
/// One sentence for the two refusals that say it — a fetch declining one Entry
/// under such a folder, and a drop or a freeze refused onto one whole — and the
/// explorer says it too, without asking: clicking into a folder no mapping
/// reaches makes no request at all. The explorer's copy is held to this one
/// through the file this crate's cases write, and it is lower-case and
/// unpunctuated at the end because the explorer sets it inside a sentence of its
/// own.
pub(crate) const NO_FOLDER_HERE_SAID: &str =
    "no folder on this device holds this part of the Library";

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
/// Entry Path, which is the user's own name for their file (spec: EL-1) and not
/// something to be echoed back out of a failure.
///
/// That second reason is why the cause is held as the redacted rendering of the
/// failure rather than as the failure itself: an Entry Path may not reach a
/// diagnostic event either, and the one way to be sure of it is to leave
/// nothing here for a diagnostic event to render. See [`redact`].
pub struct ApiError {
    status: StatusCode,
    /// Which kind of refusal this is, for the caller to branch on. It travels
    /// as `error`, and it is one of `bad_path` or `bad_request` (400),
    /// `unauthorized` (403), `no_such_entry` or `no_such_route` (404),
    /// `declined`, `refused_placement` or `epoch` (409), `locked` (423),
    /// `storage` or `unverified` (502), and `server` (500).
    ///
    /// `declined` and `refused_placement` are the two verdicts on a placement,
    /// in the Entry Path concept's words. `declined` is a fetch's verdict on one
    /// Entry: it did not place it, and the reason says why (spec: EP-11). A
    /// drop declines a file the same way where its own path cannot be placed,
    /// reporting it beside what it placed (spec: EP-4, EP-11, EP-14).
    /// `refused_placement` is the wider verdict, a placement this device will
    /// not make whether of one file or of every file under a mapped root: a
    /// mapping's root that is not the folder it was recorded against
    /// (spec: EP-13), a drop or a freeze under a folder this device has no
    /// folder for (spec: EP-9), and a drop that would replace an Entry inside a
    /// Pack (spec: PK-15).
    ///
    /// Some of them carry a second status, and none is a second kind. A
    /// request that outran what this server takes a drop within
    /// (spec: LA-9, LA-10) is `bad_request` at `413`, because what
    /// is wrong with it is its size rather than anything about the Library; a
    /// device with no room left to take a drop is `server` at `507`, because
    /// it is this machine's state and nothing the browser did; and a path this
    /// server registers, asked by a method it does not take there, is
    /// `no_such_route` at `405`, because, exactly as an unregistered path does,
    /// it asks this server for something it does not answer. `bad_request`
    /// carries two more, both about a folder on this device a caller named
    /// ([`local_folders`]): `403` for one the account this server runs as may
    /// not read, and `409` for one whose state a mapping cannot be recorded
    /// over — and the same `409` for a reconnect asked of a Library no grant
    /// reaches. A caller
    /// branching on the kind reads them as what they are and shows the
    /// sentence; one that wants the difference has the status.
    ///
    /// The whole set is named here because a browser writes a branch per kind,
    /// and a kind it has never heard of is one it falls off the end of. Adding
    /// one is adding a case to every caller.
    kind: &'static str,
    /// A user-facing explanation, written here rather than borrowed.
    message: String,
    /// Which way a placement was declined or refused, where one was:
    /// `unmapped`, `unmaterializable`, `reserved`, `surfaced`, or `key_lost`
    /// for a fetch's `declined` (spec: EP-11); `refused_root` for a mapping's root,
    /// `unmapped` for a folder this device has no folder for, and
    /// `pack_resident` for a file that would replace an Entry inside a Pack
    /// (spec: PK-10, PK-12), all three under `refused_placement`. A drop meets
    /// `unmaterializable` and `reserved` under `declined` as well. Present
    /// wherever the kind is `declined` or `refused_placement`, and the whole set
    /// for the same reason.
    ///
    /// And one that is not about a placement: `unauthenticated` under
    /// `storage`, for a credential Storage no longer takes — a grant that ran
    /// out or was revoked — which is the one Storage refusal a page answers
    /// with something other than the retry (see [`UNAUTHENTICATED`]). Every
    /// other `storage` refusal carries no reason.
    ///
    /// `reserved` is a path carrying a name coffret keeps for itself inside a
    /// mapped folder (spec: EP-11's scratch, EP-14's management area) or a name
    /// that folds to the management area's, and `refused_root` is a mapped
    /// folder that is not the folder its mapping was recorded against
    /// (spec: EP-13). Both are told apart from `unmaterializable` because what
    /// a person does about them differs: change the one name in the path that is
    /// taken, or recover the intended mapped folder — where `unmaterializable` leaves them
    /// a path no local name can stand for at all. The two readings of the
    /// management area share the reason because they share the gesture's shape —
    /// one name is the whole of what changes — and differ only in the sentence,
    /// which is the message rather than anything a caller branches on.
    ///
    /// `key_lost` is one Entry whose Container the Library records no key for
    /// (spec: KL-7), which no Passphrase remedies. It is not the server being
    /// locked, which is the `locked` *kind* above: the owner's own state, which
    /// ends when the Master Key is unlocked with the Passphrase.
    /// The two never appear together — a locked server declines nothing, because
    /// it fetches nothing.
    reason: Option<&'static str>,
    /// The finding the fetch reported, by the name the device layer gives it:
    /// `ForeignFile`, `LocallyChanged`, `WitnessedDeletion`, `UnreachablePlace`,
    /// `KeyLost`, or `ReservedComponent`.
    ///
    /// Present where the reason is `surfaced` or `key_lost`, and absent where it
    /// is `unmapped`, `unmaterializable`, `reserved`, `refused_root` or
    /// `pack_resident` — refusals no finding stands behind, because each is
    /// decided about the path or about a mapping rather than found at a place.
    /// The set is named here for the reason the others are: it is what a browser
    /// telling one declined path from another branches on.
    surfaced: Option<&'static str>,
    /// What the layer below reported, as much of it as a diagnostic event may
    /// carry ([`redact`]). For the log, and for nothing else.
    cause: Option<String>,
    /// The Entry Paths a drop had already written when this stopped it, and
    /// `None` on every refusal that is not a drop stopped part way.
    ///
    /// A drop stopped for the whole request leaves what landed before it in the
    /// folder, whole (spec: LA-10, EP-11), and a page that read only the
    /// sentence would have no way to know those files are there. So the answer
    /// says which they are, in the field an answer that took the drop names them
    /// in. They are the person's own paths in their own Library, which is who
    /// the body is read by; none of them reaches the log (spec: EL-1).
    ///
    /// A boxed slice rather than a `Vec`, because every refusal on every route
    /// carries this field and nearly none of them fills it: the list is built
    /// once and never grown, and the eight bytes a capacity would cost are
    /// what keeps this type small enough to be returned by value.
    written: Option<Box<[String]>>,
}

impl ApiError {
    fn plain(status: StatusCode, kind: &'static str, message: String) -> Self {
        Self {
            status,
            kind,
            message,
            reason: None,
            surfaced: None,
            cause: None,
            written: None,
        }
    }

    /// A placement declined, which is `409 declined` whatever declined it.
    ///
    /// The one place that kind is built, so that a `declined` cannot go out
    /// without its reason: every constructor that declines something says only
    /// which reason, which finding where one stands behind it, and what a person
    /// reads. A cause, where there is one, is added by
    /// [`caused_by`](Self::caused_by).
    fn declined_because(
        reason: &'static str,
        surfaced: Option<&'static str>,
        message: &str,
    ) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            kind: "declined",
            message: message.to_owned(),
            reason: Some(reason),
            surfaced,
            cause: None,
            written: None,
        }
    }

    /// A placement this device will not make, which is `409 refused_placement`
    /// whatever refused it.
    ///
    /// The one place that kind is built, for the reason
    /// [`declined_because`](Self::declined_because) is the one place its kind is.
    /// No finding stands behind any of these (see [`surfaced`](Self::surfaced)),
    /// so there is none to pass.
    fn refused_placement(reason: &'static str, message: String) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            kind: "refused_placement",
            message,
            reason: Some(reason),
            surfaced: None,
            cause: None,
            written: None,
        }
    }

    /// Gives a refusal that is neither `declined` nor `refused_placement` its
    /// reason — which is only ever a `storage` refusal's `unauthenticated`.
    fn because(mut self, reason: &'static str) -> Self {
        self.reason = Some(reason);
        self
    }

    /// Keeps the redacted rendering of what the layer below reported.
    fn caused_by(mut self, cause: String) -> Self {
        self.cause = Some(cause);
        self
    }

    /// Which kind of refusal this is.
    ///
    /// These four are for every caller that keeps the account of a refusal
    /// rather than answering with the refusal itself: the background fill, the
    /// sync and the freeze, which report what they met in the run they publish, and the
    /// drop route, which names the parts it refused beside what landed in an
    /// answer that is not a refusal at all. They are the four fields a
    /// refusal goes out with and no more — what a refusal never says on the
    /// wire is what the layer below reported, and that stays unreachable from
    /// here as it is unreachable from a body.
    pub(crate) fn kind(&self) -> &'static str {
        self.kind
    }

    /// Which way something was declined, where it was.
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
