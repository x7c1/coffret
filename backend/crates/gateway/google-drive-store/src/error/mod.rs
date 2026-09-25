//! What can go wrong getting this gateway ready to serve the port.
//!
//! The type and its variants are here; what each variant says is in
//! [`display`], what it says underneath in [`source`], and what it becomes
//! when it reaches the port is in [`into_port`]. A transport failure becomes
//! one in [`from`]. Every value a variant carries to say which part of a
//! folder, a token cache, a token response or a redirect failed is a type in a
//! module of its own beside them, and so is [`UnreadableAnswer`], which a
//! document Drive answered with that this build cannot read crosses the port
//! as.

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use coffret_format::Purpose;

use crate::http::TransportError;
use crate::oauth::GrantedScopes;

mod app_folder_defect;
pub use app_folder_defect::AppFolderDefect;

mod display;

mod from;

mod into_port;

mod redirect_step;
pub use redirect_step::RedirectStep;

mod source;

mod token_cache_defect;
pub use token_cache_defect::TokenCacheDefect;

mod token_response_defect;
pub use token_response_defect::TokenResponseDefect;

mod unreadable_answer;
pub(crate) use unreadable_answer::UnreadableAnswer;

#[cfg(test)]
mod tests;

/// Result alias for this crate's own fallible surface.
pub type Result<T> = std::result::Result<T, Error>;

/// What can go wrong getting this gateway ready to serve the port.
///
/// The port's own operations answer in [`coffret_usecase::Error`]; this is the
/// layer under it — building a transport, running the authorization flow,
/// keeping the token cache — which has failures of its own that no Storage
/// vocabulary would describe honestly. Where one of these surfaces during an
/// operation it is classified into the port's vocabulary, so a caller of the
/// port still only ever sees the port's error type.
///
/// A failure this layer observed as a Rust error travels as that error: the
/// value goes in a `cause`, and only the port boundary, in `into_port.rs`,
/// turns one into the port's own vocabulary. What a *remote* reported — a
/// status, a body, a message from the token endpoint — is text where it arrived
/// as text, and stays text.
#[derive(Debug)]
pub enum Error {
    /// The HTTP client could not be built.
    HttpClient {
        /// What the client library reported.
        cause: reqwest::Error,
    },
    /// The token cache could not be read or written.
    TokenCache {
        /// The file that was being read or written.
        path: PathBuf,
        /// What the operating system reported.
        cause: io::Error,
    },
    /// The token cache holds something this build cannot read.
    MalformedTokenCache {
        /// The file that was read.
        path: PathBuf,
        /// What went wrong reading it.
        cause: TokenCacheDefect,
    },
    /// The key the cache was handed was derived for another purpose, so the
    /// file was never opened (spec: KD-4).
    ///
    /// Apart from [`Error::MalformedTokenCache`] because the two are facts
    /// about different things: there the file is no good, here the caller
    /// brought the wrong key and the file is very likely fine. Reporting this
    /// as a malformed cache would send somebody to delete a credential store
    /// and authorize again over a mistake one line up the call stack.
    WrongTokenCacheKey {
        /// The file the key would have been used on.
        path: PathBuf,
        /// The purpose the key was derived for.
        actual: Purpose,
    },
    /// The tokens could not be encoded, so none of them were written.
    ///
    /// This is the step before sealing: the tokens are turned into the document
    /// that gets sealed, and a document that cannot be produced is not the file
    /// layer's failure and not the operating system's.
    UnencodableTokens {
        /// The file the tokens were meant for.
        path: PathBuf,
        /// What the encoder reported.
        cause: serde_json::Error,
    },
    /// The tokens could not be sealed, so none of them were written.
    ///
    /// Sealing is the format layer's work and its answer travels here whole:
    /// this layer sees that the cache could not be written, not why, and
    /// naming a cause it did not observe would be a guess.
    UnsealableTokenCache {
        /// The file the sealed bytes were meant for.
        path: PathBuf,
        /// What the format layer reported.
        cause: coffret_format::Error,
    },
    /// No refresh token is cached, so there is nothing to authorize calls with.
    ///
    /// The authorization flow has to be run — which needs a person at a browser
    /// — before this store can be used again.
    NotAuthorized,
    /// The provider answered the redirect saying it would not authorize.
    ///
    /// A redirect that names a refusal ends the wait rather than being waited
    /// out (spec: SA-2), once its `state` says the redirect is this flow's own
    /// — anything on the machine can aim an `error` at the loopback port, and
    /// only that check makes this the provider talking rather than
    /// [`Error::RedirectWithoutState`]. What the parameter holds is the
    /// provider's own word for why it said no — `access_denied` where the
    /// person declined — and it arrived as text, so it stays text.
    ProviderRefusedAuthorization {
        /// What the provider named as its reason.
        refusal: String,
    },
    /// The redirect did not carry the state this flow sent, so nothing says it
    /// is this flow's own callback.
    ///
    /// The `state` is what tells the browser coming back from the consent
    /// screen apart from any other page on the machine aimed at the loopback
    /// port, which makes this the CSRF check failing rather than a flow that
    /// merely went wrong (spec: SA-2).
    RedirectWithoutState,
    /// No redirect arrived before the flow stopped waiting.
    ///
    /// The person never finished at their browser, or what they finished in
    /// never came back to this machine. Nothing about the request was refused
    /// — nothing answered at all.
    RedirectTimedOut {
        /// How long the flow waited.
        after: Duration,
    },
    /// The grant carries no refresh token, so nothing durable was cached.
    ///
    /// The flow asks for one every time (`access_type=offline`,
    /// `prompt=consent`), and a provider may still withhold it — Google does on
    /// a repeat authorization where one was already issued and never revoked.
    /// The access token beside it expires within the hour, which is no use to a
    /// store that has to authorize itself unattended on the next run from the
    /// one long-lived token this flow is supposed to leave behind (spec: SA-6),
    /// so the answer is refused rather than half-cached.
    GrantWithoutRefreshToken,
    /// The grant is not [`DRIVE_FILE_SCOPE`] and nothing else, so nothing was
    /// cached.
    ///
    /// What it is about is not a message but a fact: these are the permissions
    /// the person clicked through on the consent screen, and they travel as the
    /// set the endpoint named so that whoever reports the refusal can say it
    /// their own way. Scopes are not secrets — the tokens that arrived beside
    /// them are, and none of them are carried here.
    ///
    /// [`DRIVE_FILE_SCOPE`]: crate::oauth::DRIVE_FILE_SCOPE
    GrantNotDriveFileAlone {
        /// What the endpoint said was granted, or `None` where its answer named
        /// no scope at all.
        granted: Option<GrantedScopes>,
    },
    /// The loopback the browser is redirected back to could not be run.
    ///
    /// Listening on the port, learning which one the operating system handed
    /// out, taking the browser's connection, reading what it asked for: all of
    /// it is this machine's own work, and what stopped it is what the operating
    /// system reported.
    LoopbackRedirect {
        /// The step that failed.
        step: RedirectStep,
        /// What the operating system reported.
        cause: io::Error,
    },
    /// The browser came back asking for something that is not a URL.
    MalformedRedirect {
        /// What it asked for.
        target: String,
        /// What the parser reported.
        cause: url::ParseError,
    },
    /// The token endpoint refused to issue or refresh a token.
    TokenEndpoint {
        /// The status it answered with.
        status: u16,
        /// What it reported.
        detail: String,
    },
    /// The token endpoint refused a code exchange this client made without a
    /// client secret.
    ///
    /// Apart from [`Error::TokenEndpoint`] because of the one thing the
    /// endpoint's own answer cannot say: a client registered with a secret and
    /// a code that expired are refused in the same words, and what tells them
    /// apart is on this side — the request carried no `client_secret` at all.
    /// By the time this is raised the person has been through the consent
    /// screen in their browser, so a refusal they cannot place costs them that
    /// walk again.
    ///
    /// It says the request carried no secret and stops there. Whether the
    /// client was registered with one is not something this layer can know, and
    /// where a secret would have come from is the shell's own vocabulary rather
    /// than this gateway's.
    CodeExchangeWithoutSecret {
        /// The status it answered with.
        status: u16,
        /// What it reported.
        detail: String,
    },
    /// The token endpoint answered, and the answer could not be read.
    ///
    /// Apart from [`Error::TokenEndpoint`] because the two are told apart by
    /// who observed the failure: there the endpoint said what was wrong, here
    /// nothing it said was ever recovered and what went wrong is a Rust error
    /// this layer saw. The status is still worth carrying — it says what kind
    /// of answer was being read.
    UnreadableTokenResponse {
        /// The status it answered with.
        status: u16,
        /// What went wrong reading the answer.
        cause: TokenResponseDefect,
    },
    /// A call did not become an answer this gateway could use — it did not
    /// land, or what came back is not an answer to it (see [`TransportError`]).
    Transport(TransportError),
    /// The operating system would not supply random bytes for the PKCE
    /// verifier, so no authorization request can be made safely.
    EntropyUnavailable {
        /// What the entropy source reported.
        cause: getrandom::Error,
    },
    /// The Library's app folder could not be created, so the Library has
    /// nowhere on Drive to live (FM-18).
    ///
    /// Named after the step rather than after what went wrong: this happens
    /// before any store exists, and what a caller has to know first is that it
    /// was the folder — not an object in it, and not the grant on its own —
    /// that never came into being.
    AppFolderNotCreated {
        /// The name the folder was to be created under.
        name: String,
        /// What went wrong.
        cause: AppFolderDefect,
    },
    /// Drive would not say what the folder a Library was said to live in is
    /// called, so nothing says which Library that is (FM-18).
    ///
    /// The mirror of [`AppFolderNotCreated`](Self::AppFolderNotCreated) for a
    /// device joining a Library it did not create: the folder is there, and
    /// until its name is read the id names a folder rather than a Library.
    AppFolderUnreadable {
        /// The folder that was asked about.
        folder_id: String,
        /// What went wrong.
        cause: AppFolderDefect,
    },
    /// Drive would not say what the Library's app folder holds, so nothing says
    /// whether the Library there has ever been committed to (FM-12).
    ///
    /// The second question a device joining a Library asks, after
    /// [`AppFolderUnreadable`](Self::AppFolderUnreadable)'s. That one is about
    /// identity and this one is about contents, and they are apart because
    /// their answers are: a folder whose name is not a Library's is refused,
    /// and a folder holding nothing of one is reported and joined all the same.
    /// A folder that would not answer at all is neither, which is what this
    /// says.
    LibraryObjectUnreadable {
        /// The folder that was asked about.
        folder_id: String,
        /// The object it was asked for.
        ///
        /// The call takes the name, so the failure names it too: without it
        /// two calls about the same folder for different objects leave the
        /// same line behind.
        name: String,
        /// What went wrong.
        ///
        /// Held behind a pointer, unlike the two variants above. This is the
        /// widest of the three — it names a folder *and* an object — so
        /// carrying the defect inline would leave this one variant setting the
        /// width of every `Result` this crate returns. The defect moves out of
        /// line rather than out of the error.
        cause: Box<AppFolderDefect>,
    },
}
