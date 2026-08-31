use crate::api_error::ApiError;

/// A refusal as the fill keeps it.
///
/// `ApiError` itself carries what the layer below reported, which belongs in
/// the log and nowhere else, and is not something to hold on to for as long as
/// an activity lives. What is kept is what a browser branches on and what a
/// person reads — the same four fields a refusal goes out with.
///
/// So this is what a refusal is told as, and nothing here decides anything: the
/// fill settles what a failure means while it still has the failure, and hands
/// over the account of it afterwards.
#[derive(Clone, Debug)]
pub struct Reported {
    /// Which kind of refusal this is.
    pub kind: &'static str,
    /// Which way a fetch was declined, where it was.
    pub reason: Option<&'static str>,
    /// The finding the fetch reported, by the name the device layer gives it.
    pub surfaced: Option<&'static str>,
    /// One sentence a person could read.
    pub message: String,
}

impl Reported {
    /// What one refusal says, having put what it was caused by into the log.
    ///
    /// Named for the recording because that is the part with a consequence: a
    /// refusal that reaches a response records itself on the way out, and one
    /// the fill keeps would otherwise take what the layer below reported to the
    /// grave with it.
    pub(super) fn recorded(refusal: &ApiError, operation: &'static str) -> Self {
        refusal.record(operation);
        Self {
            kind: refusal.kind(),
            reason: refusal.reason(),
            surfaced: refusal.surfaced(),
            message: refusal.message().to_owned(),
        }
    }

    /// What a fill whose worker ended without an answer is put under.
    ///
    /// Minted here rather than reported from below, because there is nothing
    /// below to report: what this stands for is the background task ending
    /// without having said how — a panic in the job — which the runtime prints
    /// where every other panic goes and which nothing in an activity can be
    /// derived from. It travels as `server`, the kind every refusal nobody
    /// outside this process can act on travels as.
    ///
    /// What it says is only that nothing finished and nothing knows why. Which
    /// folder went unfinished is said by the line it is shown in — that line
    /// names the folder and offers the retry beside it — so a message naming
    /// the folder again would put one sentence on the screen twice.
    pub(super) fn unfinished() -> Self {
        Self {
            kind: "server",
            reason: None,
            surfaced: None,
            message: "the server did not finish, and did not say why".to_owned(),
        }
    }
}
