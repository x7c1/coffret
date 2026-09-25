use axum::http::StatusCode;
use coffret_device::{EntryPath, Redacted};

use super::ApiError;

impl ApiError {
    /// A mapped folder is not the folder its mapping was recorded against
    /// (spec: EP-13).
    ///
    /// `409 declined` and a reason of its own, rather than the `500` every
    /// refusal nobody outside this process can act on travels as. It is not the
    /// server failing: the request was answerable, the Library is intact, and
    /// what is wrong is one of this device's mappings — a disk that came back
    /// empty, a mount that never came back, a folder swapped for another of the
    /// same name. A person told only that "the server could not answer" has no
    /// way to learn any of that, which is the one state EP-13 exists to keep
    /// them out of.
    ///
    /// Which of EP-13's cases it was reaches the log and not the body: it is one
    /// line beside one row, and recovery starts in the same place for every one
    /// of them. The guidance is [`refused_root_said`], which says what it leaves
    /// out and why.
    ///
    /// An upload is the one flow that cannot promise the guidance arrives: it
    /// is answered while the browser is still sending, and a transfer that
    /// fails first leaves the browser saying the server did not answer
    /// instead. The log is what carries the case in that event.
    ///
    /// The two arguments go two different ways and neither crosses. `prefix`
    /// names the mapping in the guidance, because a device with more than one
    /// leaves a person holding recovery instructions with nothing to aim them
    /// at.
    ///
    /// It takes the refusal that carried the state rather than the case inside
    /// it, so what reaches the log is that error's own redacted rendering — a
    /// fetch's and a drop's say the same case under the name of the layer that
    /// met it. Composing the line here instead would be a second spelling of a
    /// rendering those types already own, and every one of them would be filed
    /// under whichever layer this function happened to name.
    pub(crate) fn refused_root(prefix: Option<&EntryPath>, cause: &impl Redacted) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            kind: "declined",
            message: refused_root_said(prefix),
            reason: Some("refused_root"),
            surfaced: None,
            cause: Some(cause.redacted()),
        }
    }
}

/// Recovery guidance for a mapped root this device will not place into
/// (spec: EP-13).
///
/// Request refusals ([`ApiError::refused_root`]) and background findings
/// ([`Finding`](crate::Finding)) share this wording. The Library-side prefix names
/// the mapping to recover; the local filesystem path does not cross the API
/// boundary (spec: EL-1).
///
pub(crate) fn refused_root_said(prefix: Option<&EntryPath>) -> String {
    // Debug formatting quotes a named prefix; the Library-root mapping has no
    // prefix to quote.
    let mapped = match prefix {
        Some(prefix) => format!("the folder this device maps {:?} into", prefix.as_str()),
        None => "the folder this device maps the Library root into".to_owned(),
    };
    format!(
        "{mapped} is not the folder that mapping was recorded against, so nothing was put into \
         it. Open a terminal on the device serving the Library. Run `coffret mappings --library \
         <library>` to inspect the recorded mappings and `coffret map --help` to find the \
         arguments. Use that listing to choose one recovery: reconnect the intended folder if it \
         is elsewhere; if the folder at the recorded location is the intended one, record this \
         mapping again with `coffret map`; or map another folder in its place only as a deliberate \
         choice. If `coffret map` reports a local marker problem, correct the problem and run it \
         again. Then return to the explorer and try the action again"
    )
}
