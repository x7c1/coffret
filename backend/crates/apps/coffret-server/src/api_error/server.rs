//! The refusals nothing the browser did brought about: this process failing,
//! Storage handing over what does not match, and a Master Key this device no
//! longer holds.

use axum::http::StatusCode;

use super::{redact, ApiError, SERVER};

impl ApiError {
    /// A local file this device believed it had could not be read.
    pub fn unreadable(cause: std::io::Error) -> Self {
        Self::server(redact::io_failure(&cause))
    }

    /// Something the server itself could not do, whatever it was.
    ///
    /// One constructor rather than one per site, because this is the one refusal
    /// whose body says nothing about what happened — the browser did nothing and
    /// can do nothing, and what actually went wrong travels as the cause to the
    /// log. Three spellings of that sentence would be three chances for one of
    /// them to start saying more.
    pub(crate) fn server(cause: String) -> Self {
        Self::plain(
            StatusCode::INTERNAL_SERVER_ERROR,
            SERVER,
            "the server could not answer".to_owned(),
        )
        .caused_by(cause)
    }

    /// This device can no longer read or write the Library as it stands: a
    /// Master Key epoch was activated, and this device holds only the key that
    /// epoch replaced (spec: CP-5, MR-2).
    ///
    /// Its own kind, because it is the one refusal nothing on this device ends
    /// by waiting or by asking again. A writer whose slot an activation took
    /// stops until it is enrolled in the new epoch, and a device replaying past
    /// one is in the same position: everything after it is sealed under a
    /// Master Key this device was never given (spec: MR-4). Filed among
    /// Storage's failures it would be offered a retry that can only meet it
    /// again; filed among this process's it would say the server broke when
    /// nothing did.
    ///
    /// `409`, and for what it is not. Not `502`: Storage answered, and what it
    /// answered with is exactly what the Library holds. Not `500`: this process
    /// did what it was asked and read the answer correctly. Not a status for a
    /// fault in the request either, since the same request from an enrolled
    /// device is answered. What it conflicts with is the Library's current state
    /// as this device stands in it, which is what `409` says. `declined` and
    /// `refused_placement` share the status and not the meaning, and a caller
    /// tells them apart by the kind it branches on anyway. Not `423`, which is
    /// this server's own lock and is ended by the Passphrase — no Passphrase
    /// ends this.
    ///
    /// The sentence says what the person does next and nothing about which
    /// generation the activation took. The generation is Storage evidence, for
    /// the log where the cause takes it (spec: EL-5), and nobody enrolling a
    /// device again needs a number to do it.
    pub(super) fn epoch(cause: String) -> Self {
        Self::plain(
            StatusCode::CONFLICT,
            "epoch",
            "this device has to be enrolled in the Library again: the Library's Master Key was \
             replaced, and this device holds only the one before it — enroll it again with the \
             new Recovery Code"
                .to_owned(),
        )
        .caused_by(cause)
    }

    /// What arrived from Storage, or reached it, is not what the Library names
    /// or what this device sent: `502 unverified`.
    ///
    /// One constructor for every flow that meets it, so the kind is spelled
    /// once; the sentence differs with which side of the transfer failed to
    /// match, and is the caller's to say.
    pub(super) fn unverified(message: &str, cause: String) -> Self {
        Self::plain(StatusCode::BAD_GATEWAY, "unverified", message.to_owned()).caused_by(cause)
    }
}
