//! The refusals about a folder on this device a caller named: one to list the
//! folders inside, or one to map part of the Library to.
//!
//! These are the one place a refusal names a local path. A diagnostic event may
//! not carry one (spec: EL-1), and none of these gives the log anything but a
//! kind; a response may, because the folder is what the person just chose and
//! naming it is the answer they are owed. Nothing here is about the Library, so
//! a refusal of the folder is `bad_request`, at the status that says what about
//! it stopped the request; [`ApiError::mapping_refused`] gives the exceptions.

use std::io;
use std::path::Path;

use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use coffret_device::{Error, Redacted};

use super::{redact, ApiError};

impl ApiError {
    /// A folder named by anything but its whole path.
    ///
    /// A relative path has no meaning to a server whose own working directory is
    /// nothing the page knows, and a mapping recorded from one would point
    /// wherever the server happened to be started.
    pub(crate) fn not_absolute(path: &str) -> Self {
        Self::plain(
            StatusCode::BAD_REQUEST,
            "bad_request",
            format!("{path:?} is not a whole path; a folder on this device is named from /"),
        )
    }

    /// The folder a listing was asked of could not be listed, for whichever
    /// reason the operating system gave.
    ///
    /// Two of them are the caller's to act on and are said: a path that is not
    /// a folder at all is `400`, and one the account this server runs as may
    /// not read is `403`. Anything else is this device failing, and travels as
    /// the `500` with only the kind of failure for the log.
    pub(crate) fn folder_not_listed(path: &Path, cause: &io::Error) -> Self {
        match cause.kind() {
            io::ErrorKind::NotFound | io::ErrorKind::NotADirectory => Self::not_a_folder(path),
            io::ErrorKind::PermissionDenied => Self::plain(
                StatusCode::FORBIDDEN,
                "bad_request",
                format!(
                    "{} cannot be read by the account this server runs as",
                    path.display()
                ),
            ),
            _ => Self::server(redact::io_failure(cause)),
        }
    }

    /// The path names nothing on this device, or something that is not a
    /// folder.
    pub(crate) fn not_a_folder(path: &Path) -> Self {
        Self::plain(
            StatusCode::BAD_REQUEST,
            "bad_request",
            format!("{} is not a folder on this device", path.display()),
        )
    }

    /// A folder whose path this server cannot hand a page as text.
    ///
    /// A page holds a path as a string, so one whose bytes are not UTF-8 is one
    /// it could neither show nor send back; the person can still map it from a
    /// terminal.
    pub(crate) fn path_not_text(path: &Path) -> Self {
        Self::plain(
            StatusCode::BAD_REQUEST,
            "bad_request",
            format!(
                "{} cannot be shown here, because its name is not text; map it with the \
                 command line instead",
                path.display()
            ),
        )
    }

    /// No home directory to start a listing at.
    pub(crate) fn no_home() -> Self {
        Self::server("NoHome".to_owned())
    }

    /// A body that is not the JSON a route takes.
    ///
    /// The extractor's own words reach neither the body nor the log: they may
    /// quote what was sent, and what is sent to the map route is a local path.
    /// What the log is given is its status, which says which of the ways a body
    /// fails it was.
    pub(crate) fn unreadable_json(rejection: &JsonRejection) -> Self {
        Self::plain(
            StatusCode::BAD_REQUEST,
            "bad_request",
            "the request did not arrive as something this route can read".to_owned(),
        )
        .caused_by(format!(
            "JsonRejection(status={})",
            rejection.status().as_u16()
        ))
    }

    /// What a mapping the device crate would not record is answered with.
    ///
    /// The device crate's own sentence, since it is the one that decided: a
    /// prefix that is not one top-level component of the Library (spec: EP-9)
    /// is `bad_path`, a root that is not a folder on this device is
    /// `bad_request` at `400`, and a root whose management area or marker stops
    /// the identity a mapping records (spec: EP-13) is `bad_request` at `409`
    /// — the request was read, and the folder is what it conflicts with. None
    /// of those is remedied from a page except by choosing another folder, and
    /// the sentence names the folder so that the person can go and look at it.
    ///
    /// Everything else is answered the way the rest of the routes answer the
    /// same failure: a catalog that would not take the row, most of all.
    pub(crate) fn mapping_refused(error: Error) -> Self {
        let status = match &error {
            Error::MalformedMappingPrefix { cause, .. } => {
                // The model's refusal travels as the cause, and says which part
                // of the shape failed; a person told only that the prefix
                // "cannot be mapped" has nothing to correct.
                let said = match cause {
                    Some(defect) => format!("{error}: {defect}"),
                    None => error.to_string(),
                };
                return Self::plain(StatusCode::BAD_REQUEST, "bad_path", said)
                    .caused_by(error.redacted());
            }
            Error::NoSuchLocalRoot { .. } => StatusCode::BAD_REQUEST,
            Error::ManagementAreaNotADirectory { .. }
            | Error::ManagementAreaIncomplete { .. }
            | Error::ManagementAreaFolded { .. }
            | Error::MarkerNotARegularFile { .. }
            | Error::MarkerMalformed { .. }
            | Error::Local(_) => StatusCode::CONFLICT,
            _ => return Self::from(error),
        };
        Self::plain(status, "bad_request", error.to_string()).caused_by(error.redacted())
    }
}
