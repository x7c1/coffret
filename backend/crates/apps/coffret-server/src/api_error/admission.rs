//! The refusals made before a route has read the request: who is answered at
//! all, whether the Library is open, and whether this server answers the path.

use axum::http::StatusCode;

use super::ApiError;

impl ApiError {
    /// The request is not one this server answers, whoever sent it.
    ///
    /// One of the two refusals made before a route is reached, and the one
    /// about the caller — so it is never about the Library: nothing in it says
    /// whether the path exists, whether the folder is mapped, or whether
    /// anything at all was asked for. `403` rather than `401`, because there is no challenge to
    /// answer here — the key is read off this device's disk, and a caller that
    /// cannot read it has nothing to try again with.
    ///
    /// The sentence is taken as anything that becomes one rather than as a
    /// literal, because one of the admission fences' three sentences names the
    /// address this server bound and the rest are fixed text.
    pub(crate) fn unauthorized(message: impl Into<String>) -> Self {
        Self::plain(StatusCode::FORBIDDEN, "unauthorized", message.into())
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

    /// This server answers nothing at the path that was asked for.
    ///
    /// The other refusal made before a route is reached, and made because none
    /// was: the path is none of the ones [`router`](crate::router::router)
    /// registers. Answered in the one shape all the same, because what reads it
    /// is a caller that did reach this server — a page of an older build asking
    /// for a route since renamed, most of the time — and a body it cannot parse
    /// is one it can only read as something else having replied in this
    /// server's place, which would be false.
    ///
    /// One kind at two statuses, the way `bad_request` is at `400` and `413`:
    /// this is the `404`, and [`no_such_method`](Self::no_such_method) is the
    /// `405` for a path that is registered and was asked by a method it does not
    /// take. What a caller does about either is the same — it asked for
    /// something this server does not answer — and one that wants the
    /// difference has the status.
    ///
    /// The sentence repeats neither the path nor the method. What
    /// [`unauthorized`](Self::unauthorized) holds to for the same reason holds
    /// here: a refusal made before any route has read the request speaks about
    /// this server, and echoes nothing of the request back out of it.
    pub(crate) fn no_such_route() -> Self {
        Self::plain(
            StatusCode::NOT_FOUND,
            "no_such_route",
            "this server answers nothing at that path".to_owned(),
        )
    }

    /// This server answers the path that was asked for, but not by the method it
    /// was asked by.
    ///
    /// The `405` of [`no_such_route`](Self::no_such_route), which says why the two
    /// are one kind and why neither sentence names the request.
    pub(crate) fn no_such_method() -> Self {
        Self::plain(
            StatusCode::METHOD_NOT_ALLOWED,
            "no_such_route",
            "this server answers that path, but not by that method".to_owned(),
        )
    }
}
