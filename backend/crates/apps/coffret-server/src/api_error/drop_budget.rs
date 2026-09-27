//! The refusals about a drop as a request: a body this route cannot read, the
//! budgets it is taken within, and the room this device has for it.

use axum::extract::multipart::MultipartError;
use axum::http::StatusCode;

use super::{redact, ApiError, SERVER};

impl ApiError {
    /// The request itself is not one this route can read.
    ///
    /// Kept apart from [`bad_path`](Self::bad_path), which is about a path a
    /// caller named: this is the framing around it — a multipart body that ends
    /// mid-part, a boundary that is not one. There is nothing about the Library
    /// in it, and nothing for a screen to say beyond that the request did not
    /// arrive whole.
    pub fn bad_request(cause: &MultipartError) -> Self {
        Self::plain(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "the request did not arrive as something this route can read".to_owned(),
        )
        .caused_by(redact::multipart(cause))
    }

    /// A multipart body that could not be read, or that outran the body limit
    /// the route is mounted with (spec: LA-9).
    ///
    /// One constructor for both because both are the same thing said about one
    /// multipart body: the request did not arrive as one this route takes.
    /// Which of the two it was is the status, and the status is the extractor's
    /// own verdict rather than a second reading here — it is the half that knows
    /// whether it stopped because the boundary was wrong or because the bytes
    /// ran past what it was allowed to read.
    pub fn multipart(cause: MultipartError) -> Self {
        match cause.status() == StatusCode::PAYLOAD_TOO_LARGE {
            true => Self::whole_drop_too_large().caused_by(redact::multipart(&cause)),
            false => Self::bad_request(&cause),
        }
    }

    /// The whole request passed the one budget no single file in it did
    /// (spec: LA-9, LA-10).
    ///
    /// Named apart because it is said in two places: here, as the body limit is
    /// met, and by the explorer, which refuses a drop it can already tell is
    /// past the budget before sending it. It says this sentence there, read from
    /// the file this crate's cases hold to what is written here, so a person
    /// reads the server's words whichever side found the drop too large.
    pub(crate) fn whole_drop_too_large() -> Self {
        Self::too_large(
            "the drop as a whole is what passed that, rather than any one file in it — the \
             same files in two drops are taken",
        )
    }

    /// The request passed one of the budgets the server takes a drop within
    /// (spec: LA-9, LA-10).
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
    /// folder as the whole files they are — no file becomes visible before it is
    /// complete (spec: EP-11) — and nothing is armed for them. Nothing on this
    /// server arms one on its own either: they wait in the folder the way
    /// anything else copied into a mapped folder waits, until a later drop that
    /// lands something arms a flow, or somebody asks for one. Both flows take
    /// them up: a sync walks the mapped folders and finds them, and a freeze
    /// packs everything under the folder it was armed on (spec: PK-17), these
    /// files included where that is the folder they are waiting in.
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
            SERVER,
            "this device has not the room to take these files: the volume its folder for this \
             part of the Library is on is nearly full — free some room on it and drop them \
             again"
                .to_owned(),
        )
    }

    /// Says which files a drop had written when this stopped it, empty where
    /// nothing had landed yet, which is an answer too.
    pub(crate) fn having_written(mut self, written: Vec<String>) -> Self {
        self.written = Some(written.into_boxed_slice());
        self
    }
}
