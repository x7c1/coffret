use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use coffret_format::Purpose;

use coffret_usecase::GatewayFailure;

use crate::http::TransportError;
use crate::oauth::{GrantedScopes, DRIVE_FILE_SCOPE};

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
/// value goes in a `cause`, and only the port boundary below turns one into the
/// port's own vocabulary. What a *remote* reported — a status, a body, a
/// message from the token endpoint — is text where it arrived as text, and
/// stays text.
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

/// What kept an app folder from being created, read, or looked into.
///
/// As with [`TokenCacheDefect`], every one of them is one verdict to a caller —
/// there is no folder to work in — and they are kept apart so that whichever
/// layer saw the failure has its own answer travel whole. None of that depends
/// on how many there are, and saying the number here would be one more thing
/// for whoever adds the next variant to keep true.
#[derive(Debug)]
pub enum AppFolderDefect {
    /// The call never succeeded: Drive refused it, or its answer never arrived
    /// whole. It comes classified — whether trying again could help is carried
    /// in it.
    Call(coffret_usecase::Error),
    /// Drive answered, and the answer is not the file resource this build
    /// expects, so nothing in it names a folder.
    Answer(serde_json::Error),
    /// Drive answered with a file resource carrying no name, though the name is
    /// the one field the call asked for.
    Nameless,
    /// Drive kept handing back somewhere to carry on and never said the listing
    /// was over, so the walk stopped instead of following it forever.
    ///
    /// Apart from [`Call`](Self::Call) and [`Answer`](Self::Answer) by what was
    /// observed: every call succeeded and every answer read as a listing, and
    /// what is wrong is the sequence of them. Apart from
    /// [`Nameless`](Self::Nameless), which this layer also puts together
    /// itself, by what it is about — a field Drive left out of one answer,
    /// against a walk over many that never reached an end. A provider that
    /// answers an empty page and another continuation without end would
    /// otherwise leave the call spinning, which a person sees as a command that
    /// never returns.
    UnendingListing {
        /// How many pages were read before the walk gave up.
        pages: usize,
    },
}

impl fmt::Display for AppFolderDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Which of the three it was, and not what the call or the reader
            // answered: `source` hands that value on, and a caller printing
            // the chain reads it under this line rather than twice over.
            //
            // This one is about the call and the two below are about an
            // answer, which is the whole of what tells them apart: Drive
            // refusing outright and Drive sending back something this build
            // cannot read are different things to be told. A line saying the
            // call was not answered with a folder would be true of all three
            // and would leave a person guessing which had happened.
            Self::Call(_) => f.write_str("the call to Drive did not succeed"),
            Self::Answer(_) => {
                f.write_str("the answer is not the file resource this build expects")
            }
            Self::Nameless => {
                f.write_str("the answer carries no name, which is what was asked for")
            }
            // The one arm whose line is about the listing rather than about a
            // call or an answer, so it says how far the walk got: the number
            // is the whole of what a reader can check the verdict against.
            Self::UnendingListing { pages } => {
                write!(f, "the listing did not end within {pages} pages")
            }
        }
    }
}

impl error::Error for AppFolderDefect {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Call(cause) => Some(cause),
            Self::Answer(cause) => Some(cause),
            // Nothing a Rust error reported: a name Drive left out and a
            // listing that would not end are both facts this layer put
            // together itself.
            Self::Nameless | Self::UnendingListing { .. } => None,
        }
    }
}

/// What made a cached token file unreadable.
///
/// The two are one verdict to a caller — the cache is no good, so authorize
/// again — and are kept apart only so that whichever layer saw the failure has
/// its own answer travel whole, rather than a message composed here about work
/// this layer did not do.
#[derive(Debug)]
pub enum TokenCacheDefect {
    /// The sealed form could not be opened: another Master Key wrote it, its
    /// bytes have been edited, or it was never a sealed cache at all.
    ///
    /// Every one of those is a fact about the file, which is what makes this a
    /// verdict on the cache. The one thing the format layer refuses that is a
    /// fact about the *caller* — a key derived for another purpose — never
    /// reaches here: [`Error::WrongTokenCacheKey`] is raised before the file is
    /// so much as opened.
    Sealed(coffret_format::Error),
    /// The sealed form opened, and what was inside is not the token document
    /// this build expects.
    Document(serde_json::Error),
}

impl fmt::Display for TokenCacheDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Which of the two it was, and not what the format layer or the
            // reader answered: `source` hands that value on, and a caller
            // printing the chain reads it under this line.
            Self::Sealed(_) => f.write_str("the sealed form could not be opened"),
            Self::Document(_) => {
                f.write_str("what was inside is not the token document this build expects")
            }
        }
    }
}

impl error::Error for TokenCacheDefect {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Sealed(cause) => Some(cause),
            Self::Document(cause) => Some(cause),
        }
    }
}

/// What kept the token endpoint's answer from being read.
///
/// As with [`TokenCacheDefect`], the two are one verdict to a caller — nothing
/// usable came back from the endpoint — and are kept apart so that whichever
/// layer saw the failure has its own answer travel whole.
#[derive(Debug)]
pub enum TokenResponseDefect {
    /// The body never arrived whole: the transfer broke, or it was not as
    /// long as the answer declared.
    Body(coffret_usecase::Error),
    /// The body arrived, and what it holds is not the token document this
    /// build expects.
    Document(serde_json::Error),
}

impl fmt::Display for TokenResponseDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Which of the two it was, for the reason [`TokenCacheDefect`]'s
            // two say only which of theirs it was.
            Self::Body(_) => f.write_str("the body never arrived whole"),
            Self::Document(_) => {
                f.write_str("what the body holds is not the token document this build expects")
            }
        }
    }
}

impl error::Error for TokenResponseDefect {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Body(cause) => Some(cause),
            Self::Document(cause) => Some(cause),
        }
    }
}

/// A document Drive answered with that is not one this build can read.
///
/// The value a parse that failed crosses the port as. What was being read is
/// this gateway's own line, and the parser's refusal is the link under it, so
/// a chain walked from the port reads both once each. Handing over the
/// parser's error alone would drop what was being read from the chain — the
/// port prints no `detail` where a value stands behind it — and keeping that
/// in a `detail` rendered with the parser's message beside it would say
/// again, in a field, what the next link already says.
#[derive(Debug)]
pub(crate) struct UnreadableAnswer {
    /// What was being read, as the words after "unreadable".
    about: String,
    /// What the parser refused.
    cause: serde_json::Error,
}

impl UnreadableAnswer {
    pub(crate) fn new(about: impl Into<String>, cause: serde_json::Error) -> Self {
        Self {
            about: about.into(),
            cause,
        }
    }

    /// The port's word for an answer this build cannot read, with this value
    /// behind it and its own top line as the `detail`.
    pub(crate) fn into_port(self) -> coffret_usecase::Error {
        coffret_usecase::Error::MalformedResponse {
            detail: self.to_string(),
            source: Some(GatewayFailure::new(self)),
        }
    }
}

impl fmt::Display for UnreadableAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unreadable {}", self.about)
    }
}

impl error::Error for UnreadableAnswer {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.cause)
    }
}

/// Which call of the loopback redirect a failure happened at.
///
/// The operating system reports what went wrong, never what was being asked
/// of it, and that is what separates a port already taken from a browser that
/// hung up before it said anything.
#[derive(Debug)]
pub enum RedirectStep {
    /// Listening on the loopback port the browser is to be sent back to.
    Bind,
    /// Reading back which port the operating system handed out.
    Port,
    /// Taking the connection the browser arrives on.
    Accept,
    /// Reading the request the browser sent.
    Read,
}

impl fmt::Display for RedirectStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Bind => "could not listen for the redirect",
            Self::Port => "could not read the redirect port",
            Self::Accept => "could not accept the redirect",
            Self::Read => "could not read the redirect",
        })
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Every arm below that has a `cause` says which step of this
            // layer's work refused and what it was working on, and stops
            // there. What the library, the operating system, the format layer
            // or the layer under this one answered is the value `source` hands
            // on, which a caller walking the chain prints underneath —
            // rendering it in here as well would spell one refusal twice over.
            Self::HttpClient { .. } => f.write_str("could not build an HTTP client"),
            Self::TokenCache { path, .. } => {
                write!(f, "could not use the token cache at {path:?}")
            }
            Self::MalformedTokenCache { path, .. } => {
                write!(f, "the token cache at {path:?} is unreadable")
            }
            Self::WrongTokenCacheKey { path, actual } => write!(
                f,
                "the token cache at {path:?} needs the {} key, and the key given \
                 was derived for {actual}",
                Purpose::TokenCache
            ),
            Self::UnencodableTokens { path, .. } => {
                write!(f, "could not encode the tokens for the cache at {path:?}")
            }
            Self::UnsealableTokenCache { path, .. } => {
                write!(f, "could not seal the token cache at {path:?}")
            }
            Self::NotAuthorized => {
                f.write_str("no refresh token is cached; run the authorization flow first")
            }
            Self::ProviderRefusedAuthorization { refusal } => write!(
                f,
                "authorization did not complete: the request was refused: {refusal}"
            ),
            Self::RedirectWithoutState => f.write_str(
                "authorization did not complete: the redirect did not carry the state \
                 this flow sent",
            ),
            Self::RedirectTimedOut { after } => write!(
                f,
                "authorization did not complete: no redirect arrived within {}s",
                after.as_secs()
            ),
            Self::GrantWithoutRefreshToken => f.write_str(
                "authorization did not complete: the grant carries no refresh token, so \
                 nothing would outlive this run",
            ),
            Self::GrantNotDriveFileAlone {
                granted: Some(granted),
            } => write!(
                f,
                "authorization did not complete: the grant is not {DRIVE_FILE_SCOPE} alone, \
                 but {granted}"
            ),
            Self::GrantNotDriveFileAlone { granted: None } => write!(
                f,
                "authorization did not complete: the token endpoint named no scope, so nothing \
                 says the grant is {DRIVE_FILE_SCOPE} alone"
            ),
            Self::LoopbackRedirect { step, .. } => {
                write!(f, "authorization did not complete: {step}")
            }
            Self::MalformedRedirect { target, .. } => {
                write!(
                    f,
                    "authorization did not complete: the redirect target {target:?} is not a URL"
                )
            }
            Self::TokenEndpoint { status, detail } => {
                write!(f, "the token endpoint answered {status}: {detail}")
            }
            Self::CodeExchangeWithoutSecret { status, detail } => write!(
                f,
                "the token endpoint answered {status} to a code exchange made without a \
                 client secret, which a client registered with one cannot be authorized \
                 without: {detail}"
            ),
            Self::UnreadableTokenResponse { status, .. } => {
                write!(
                    f,
                    "the token endpoint answered {status}: unreadable token response"
                )
            }
            Self::Transport(_) => f.write_str("a call did not become a usable answer"),
            Self::EntropyUnavailable { .. } => f.write_str("could not draw random bytes"),
            Self::AppFolderNotCreated { name, .. } => {
                write!(f, "could not create the app folder {name:?}")
            }
            Self::AppFolderUnreadable { folder_id, .. } => {
                write!(f, "could not read the folder {folder_id:?}")
            }
            Self::LibraryObjectUnreadable {
                folder_id, name, ..
            } => write!(
                f,
                "could not tell whether the folder {folder_id:?} holds {name:?}"
            ),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::HttpClient { cause } => Some(cause),
            Self::TokenCache { cause, .. } => Some(cause),
            Self::MalformedTokenCache { cause, .. } => Some(cause),
            Self::UnencodableTokens { cause, .. } => Some(cause),
            Self::UnsealableTokenCache { cause, .. } => Some(cause),
            Self::UnreadableTokenResponse { cause, .. } => Some(cause),
            Self::LoopbackRedirect { cause, .. } => Some(cause),
            Self::MalformedRedirect { cause, .. } => Some(cause),
            Self::Transport(error) => Some(error),
            Self::EntropyUnavailable { cause } => Some(cause),
            Self::AppFolderNotCreated { cause, .. } | Self::AppFolderUnreadable { cause, .. } => {
                Some(cause)
            }
            Self::LibraryObjectUnreadable { cause, .. } => Some(cause.as_ref()),
            // Nothing a Rust error reported: what these carry is what a remote
            // said, or a fact this layer put together itself — that nothing was
            // cached, that nothing came back in time, that the key handed over
            // was for another purpose.
            Self::NotAuthorized
            | Self::WrongTokenCacheKey { .. }
            | Self::ProviderRefusedAuthorization { .. }
            | Self::RedirectWithoutState
            | Self::RedirectTimedOut { .. }
            | Self::GrantWithoutRefreshToken
            | Self::GrantNotDriveFileAlone { .. }
            | Self::TokenEndpoint { .. }
            | Self::CodeExchangeWithoutSecret { .. } => None,
        }
    }
}

impl From<TransportError> for Error {
    fn from(error: TransportError) -> Self {
        Self::Transport(error)
    }
}

/// Whether a link in a chain is one this workspace wrote.
///
/// Listed by type rather than decided by a trait, because being this
/// workspace's own is not something a foreign error could be asked. Every type
/// that can stand in a chain rooted at this crate's [`Error`] is here: this
/// crate's own vocabularies, and the three layers below whose errors reach
/// them. One left out is read as foreign, which stops the walk a link early
/// rather than carrying something it should not.
fn is_this_workspaces_own(error: &(dyn error::Error + 'static)) -> bool {
    error.is::<Error>()
        || error.is::<AppFolderDefect>()
        || error.is::<TokenCacheDefect>()
        || error.is::<TokenResponseDefect>()
        || error.is::<TransportError>()
        || error.is::<coffret_format::Error>()
        || error.is::<coffret_usecase::Error>()
        || error.is::<coffret_model::Error>()
}

/// `error`'s chain down to the first error this workspace did not write, that
/// one's own sentence included, joined the way the binaries print a chain.
///
/// The name says where it stops, because stopping is the whole of what
/// distinguishes it from the walk the examples keep (`every_link` in
/// `examples/support/mod.rs`), which follows a chain all the way down.
///
/// It is the message of the `io::Error` a local failure crosses the port in,
/// and only that. The port's `Io` carries what the operating system reported
/// rather than this crate's error, so the one place this layer's account of a
/// local failure can travel is that error's own message: `coffret-cli` and
/// `coffret-server` end on `eprintln!("{error:#}")`, which spells a chain as
/// its links separated by `": "`, so a message built this way reads to a person
/// exactly as the chain would have. Every other failure crosses with this
/// crate's error itself as the port's `source`, and a chain walked from there
/// needs nothing rendered in advance.
///
/// It stops at the first foreign link because a foreign `Display` renders
/// itself and not its sources, which is where the flattening this crate's
/// wrappers used to do stopped anyway.
fn chain_to_the_workspace_edge(error: &(dyn error::Error + 'static)) -> String {
    let mut rendered = error.to_string();
    let mut below = error.source();
    while let Some(link) = below {
        rendered.push_str(": ");
        rendered.push_str(&link.to_string());
        if !is_this_workspaces_own(link) {
            break;
        }
        below = link.source();
    }
    rendered
}

/// What a failure about a Library's app folder says, read before the error
/// holding it is handed across whole.
enum FolderVerdict {
    /// A call that failed, already classified by the same code every other
    /// Drive call goes through.
    Classified(coffret_usecase::Error),
    /// Drive answered, and the answer names no folder this build can read.
    Unreadable,
    /// Drive answered every page and never said the listing was over.
    Unending {
        /// How many pages were read before the walk gave up.
        pages: usize,
    },
}

impl FolderVerdict {
    fn of(defect: &AppFolderDefect) -> Self {
        match defect {
            AppFolderDefect::Call(cause) => Self::Classified(cause.clone()),
            AppFolderDefect::Answer(_) | AppFolderDefect::Nameless => Self::Unreadable,
            AppFolderDefect::UnendingListing { pages } => Self::Unending { pages: *pages },
        }
    }

    /// How a failure about a Library's app folder reads in the port's
    /// vocabulary.
    ///
    /// The folder this was about is named in the typed error a caller of that
    /// operation gets first. A call that failed travels as it was classified
    /// rather than being flattened into a message about a folder. A listing
    /// that never ended is the port's own [`ListingPastCap`], because every
    /// page of it was answered and read: calling it an answer this build cannot
    /// read would send somebody looking for a response that does not exist.
    ///
    /// [`ListingPastCap`]: coffret_usecase::Error::ListingPastCap
    fn into_port(self, error: Error, detail: String) -> coffret_usecase::Error {
        match self {
            Self::Classified(cause) => cause,
            Self::Unreadable => coffret_usecase::Error::MalformedResponse {
                detail,
                source: Some(GatewayFailure::new(error)),
            },
            Self::Unending { pages } => coffret_usecase::Error::ListingPastCap {
                pages,
                source: Some(GatewayFailure::new(error)),
            },
        }
    }
}

impl From<Error> for coffret_usecase::Error {
    fn from(error: Error) -> Self {
        // The error itself crosses as `source`, because
        // [`coffret_usecase::Error`] lives in the domain and cannot name this
        // type, and what is not handed over whole is gone. The port's line for
        // the kind of failure is printed above it and the chain under it, so
        // `detail` carries only this error's own top line — for a caller that
        // reads the field without walking — and rendering the chain into it as
        // well would say every sentence below twice.
        //
        // A local failure crosses as `Io`, which carries the operating
        // system's error rather than this one, so there the message has to
        // hold the whole chain or lose it (`chain_to_the_workspace_edge`).
        //
        // The arms match in place and bind by reference, so that the error is
        // still whole to hand over once its variant has been read.
        let detail = error.to_string();
        match error {
            // Nothing about the request is wrong; there is simply no usable
            // credential, and no number of retries will produce one.
            Error::NotAuthorized
            | Error::ProviderRefusedAuthorization { .. }
            | Error::RedirectWithoutState
            | Error::RedirectTimedOut { .. }
            | Error::GrantWithoutRefreshToken
            | Error::GrantNotDriveFileAlone { .. }
            | Error::MalformedRedirect { .. }
            | Error::TokenEndpoint { .. }
            | Error::CodeExchangeWithoutSecret { .. } => Self::Unauthenticated {
                detail,
                source: Some(GatewayFailure::new(error)),
            },
            // A token response whose body broke off in transit says nothing
            // about the grant: the endpoint was answering, and the connection
            // under it gave out. Folding it into `Unauthenticated` told the
            // person to renew a grant that was never in question, and told the
            // retry loop to give up on a call that the next attempt may well
            // complete — the same call failing before its body began already
            // crosses as a transport failure, through `Transport` below. So
            // whether it is worth another attempt is read off the verdict the
            // body's own drain reached, which is already in the port's
            // vocabulary: a transfer that broke is `Transport`, and one refused
            // for what it was — longer than any token response can be — is an
            // answer this build cannot read. A drain into memory has no local
            // end of its own, so an `Io` out of it is the connection's reader
            // giving out, and is a transfer that broke like the rest.
            Error::UnreadableTokenResponse {
                cause: TokenResponseDefect::Body(ref drained),
                ..
            } if drained.is_retryable() || matches!(drained, coffret_usecase::Error::Io { .. }) => {
                Self::Transport {
                    detail,
                    source: Some(GatewayFailure::new(error)),
                }
            }
            // A token response that arrived whole and is not the document this
            // build expects is the endpoint saying yes in a shape nobody here
            // can read — a mint is the only answer this is raised for — which
            // is not a verdict on the grant either. It is an answer this build
            // cannot read, and like every such answer it is not retried.
            Error::UnreadableTokenResponse { .. } => Self::MalformedResponse {
                detail,
                source: Some(GatewayFailure::new(error)),
            },
            Error::Transport(transport) => transport.into(),
            // Nothing is wrong with the credential or the request: the local
            // machine could not do its part of the work. The port carries an
            // `io::Error`, so the kind the operating system reported is kept —
            // it is what a caller acts on — while the message this layer
            // composed, which names the file, becomes that error's own.
            //
            // The loopback the browser comes back to is this machine's own work
            // in exactly that way. It crossed as `Unauthenticated` once, which
            // reads as Storage having rejected the credentials when Storage was
            // never asked anything: a port already taken is the operating
            // system's answer, and the kind it gave is what says so.
            Error::TokenCache { ref cause, .. } | Error::LoopbackRedirect { ref cause, .. } => {
                Self::Io {
                    cause: Arc::new(io::Error::new(
                        cause.kind(),
                        chain_to_the_workspace_edge(&error),
                    )),
                }
            }
            // A cache this build cannot read is this machine's own file too,
            // and it crosses beside the one the operating system refused rather
            // than as a verdict about the credentials. `Io` is the variant whose
            // rendering in a log is the kind alone, and the message this layer
            // composed names the file. What the person is to do about it is
            // unchanged and is said where they read it, in that message. The
            // file was read and what came out of it is what is wrong, so there
            // is no kind to keep.
            Error::MalformedTokenCache { .. } => Self::Io {
                cause: Arc::new(io::Error::other(chain_to_the_workspace_edge(&error))),
            },
            // Local failures the operating system was never asked about: the
            // port names every failure of this machine's own part `Io`, and
            // there is no kind to keep. A key derived for another purpose is
            // one of them and not a verdict on the credentials — nothing about
            // the grant has been looked at when this is raised.
            Error::UnencodableTokens { .. }
            | Error::UnsealableTokenCache { .. }
            | Error::WrongTokenCacheKey { .. }
            | Error::EntropyUnavailable { .. } => Self::Io {
                cause: Arc::new(io::Error::other(chain_to_the_workspace_edge(&error))),
            },
            Error::HttpClient { .. } => Self::Unsupported {
                detail,
                source: Some(GatewayFailure::new(error)),
            },
            Error::AppFolderNotCreated { ref cause, .. }
            | Error::AppFolderUnreadable { ref cause, .. } => {
                FolderVerdict::of(cause).into_port(error, detail)
            }
            // The defect is boxed in this one variant and in no other, so it is
            // read through the box the same way.
            Error::LibraryObjectUnreadable { ref cause, .. } => {
                FolderVerdict::of(cause).into_port(error, detail)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use coffret_model::Redacted;

    use super::*;
    use crate::test_support::chain;

    /// A `serde_json::Error` like the one a failed encode would hand over.
    fn json_error() -> serde_json::Error {
        serde_json::from_str::<serde_json::Value>("{").expect_err("the document is truncated")
    }

    fn path() -> PathBuf {
        PathBuf::from("/home/someone/.config/coffret/tokens.bin")
    }

    // An answer that did not parse crosses with what was being read as its own
    // link and the parser's refusal under it: each sentence once in the chain,
    // and the `detail` a caller reads without walking is that link's line
    // rather than the chain rendered into a field.
    #[test]
    fn an_answer_that_does_not_parse_says_what_was_being_read_once() {
        let parser = json_error().to_string();

        let error = UnreadableAnswer::new("file resource", json_error()).into_port();

        let coffret_usecase::Error::MalformedResponse { detail, .. } = &error else {
            panic!("an answer that does not parse is one this build cannot read: {error:?}");
        };
        assert_eq!(detail, "unreadable file resource");
        assert_eq!(
            chain(&error),
            [
                "could not read Storage's answer".to_owned(),
                "unreadable file resource".to_owned(),
                parser,
            ],
        );
    }

    #[test]
    fn a_refused_cache_file_reaches_the_port_carrying_its_kind() {
        let error = Error::TokenCache {
            path: path(),
            cause: io::Error::new(io::ErrorKind::PermissionDenied, "Permission denied"),
        };

        let coffret_usecase::Error::Io { cause } = coffret_usecase::Error::from(error) else {
            panic!("a cache the operating system refused is a local failure");
        };
        assert_eq!(cause.kind(), io::ErrorKind::PermissionDenied);
        // The file it happened to is in the message, where a reader needs it,
        // and so is what the operating system said about it: the port's `Io`
        // carries this one `io::Error` and nothing under it.
        assert_eq!(
            cause.to_string(),
            "could not use the token cache at \"/home/someone/.config/coffret/tokens.bin\": \
             Permission denied",
        );
    }

    // Tokens that cannot be encoded are this machine's failure, not a reason to
    // send anybody to look at their authorization.
    #[test]
    fn tokens_that_cannot_be_encoded_reach_the_port_as_a_local_failure() {
        let error = Error::UnencodableTokens {
            path: path(),
            cause: json_error(),
        };

        assert!(error::Error::source(&error).is_some());
        assert!(matches!(
            coffret_usecase::Error::from(error),
            coffret_usecase::Error::Io { .. }
        ));
    }

    // A cache this build cannot read is one of this device's own files, and the
    // message this layer composed about it names that file. Crossing as a local
    // failure is what keeps the name out of the rendering a diagnostic event
    // is built from, while leaving it in the message a person reads.
    #[test]
    fn either_defect_in_a_cache_reaches_the_port_as_a_local_failure() {
        let defects = [
            TokenCacheDefect::Sealed(coffret_format::Error::AuthenticationFailed),
            TokenCacheDefect::Document(json_error()),
        ];
        for cause in defects {
            let error = Error::MalformedTokenCache {
                path: path(),
                cause,
            };
            assert!(error::Error::source(&error).is_some());

            let crossed = coffret_usecase::Error::from(error);
            assert!(
                matches!(crossed, coffret_usecase::Error::Io { .. }),
                "{crossed:?}"
            );
            // The port's own line says which layer refused and the message
            // this layer composed is the link under it, which is where a
            // person reading `{error:#}` meets the file.
            let said = chain(&crossed);
            assert_eq!(
                said.first().map(String::as_str),
                Some("local transfer failed")
            );
            assert!(
                said.iter().any(|link| link.contains("tokens.bin")),
                "{said:?}"
            );
            assert_eq!(crossed.redacted(), "Io(kind=Other)");
        }
    }

    // The `Io` mapping above is chosen deliberately and explained at length,
    // and what it costs is that the port says "local transfer failed" about a
    // credential store — so the choice is pinned here rather than only
    // described. A reader changing it to `Unauthenticated` would be changing
    // the sentence a person reads, and this is where that shows up.
    #[test]
    fn an_unreadable_cache_reads_as_this_machines_own_failure_at_the_port() {
        let error = Error::MalformedTokenCache {
            path: path(),
            cause: TokenCacheDefect::Sealed(coffret_format::Error::AuthenticationFailed),
        };

        let crossed = coffret_usecase::Error::from(error);
        let rendered = chain(&crossed).join("; ");
        assert!(rendered.starts_with("local transfer failed"), "{rendered}");
        assert!(
            !rendered.contains("Storage rejected the credentials"),
            "the file is this device's own, and Storage was never asked: {rendered}"
        );
        // A file that will not open opens no better on a second read, so no
        // retry loop is to spend an attempt on it.
        assert!(!crossed.is_retryable(), "{rendered}");
    }

    // The mirror of it: a key that was derived for another purpose crosses the
    // same way, because nothing about the credential has been looked at when it
    // is raised — the file was never opened.
    #[test]
    fn a_key_for_another_purpose_reaches_the_port_as_a_local_failure() {
        let error = Error::WrongTokenCacheKey {
            path: path(),
            actual: Purpose::ControlJournal,
        };
        // Nothing a Rust error reported: the fact is which purpose the key
        // carried, and that is what travels.
        assert!(error::Error::source(&error).is_none());

        let crossed = coffret_usecase::Error::from(error);
        assert!(
            matches!(crossed, coffret_usecase::Error::Io { .. }),
            "{crossed:?}"
        );
        assert!(!crossed.is_retryable(), "{crossed}");
        // The file it would have been used on is in the chain a person reads
        // and out of the rendering an event is built from (spec: EL-1).
        let said = chain(&crossed);
        assert!(
            said.iter().any(|link| link.contains("tokens.bin")),
            "{said:?}"
        );
        assert_eq!(crossed.redacted(), "Io(kind=Other)");
    }

    // The wrapper's own line says which answer could not be read, and which of
    // the two defects it was — and, under that, what the layer below answered —
    // is what the chain carries. The `detail` that crosses is the wrapper's
    // own line, and every link under it is in the chain walked from the port,
    // once, because the error itself crosses as the port's `source`.
    //
    // The body defect is a port error standing in the middle of a chain rather
    // than at the end of one: the drain that reads the answer hands back the
    // port's vocabulary, and the operating system's own answer hangs under
    // that. It is the deepest a chain out of this crate reaches, and the case
    // that holds the chain walked from the port to following that vocabulary
    // through rather than stopping at it.
    //
    // Neither defect is a verdict on the grant. A body that broke off is a
    // transfer that broke, and worth the next attempt; a token document this
    // build cannot read is an answer it cannot read. Neither sends somebody to
    // authorize again.
    #[test]
    fn neither_defect_in_an_answer_reaches_the_port_as_a_rejected_grant() {
        let defects = [
            TokenResponseDefect::Body(coffret_usecase::Error::from(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the stream ended early",
            ))),
            TokenResponseDefect::Document(json_error()),
        ];
        for cause in defects {
            let said = chain(&cause);
            let broke_off = matches!(cause, TokenResponseDefect::Body(_));
            let error = Error::UnreadableTokenResponse { status: 200, cause };
            assert!(error.to_string().contains("answered 200"), "{error}");
            assert!(error::Error::source(&error).is_some());

            let top = error.to_string();
            let crossed = coffret_usecase::Error::from(error);
            let detail = match &crossed {
                coffret_usecase::Error::Transport { detail, .. } if broke_off => detail,
                coffret_usecase::Error::MalformedResponse { detail, .. } if !broke_off => detail,
                other => panic!("an unreadable token response is not {other:?}"),
            };
            assert_eq!(crossed.is_retryable(), broke_off, "{crossed}");
            // The field says this layer's own line, for a caller that reads
            // it without walking; every link beneath is in the chain, once.
            assert_eq!(detail, &top);
            let links = chain(&crossed);
            for link in &said {
                assert_eq!(
                    links
                        .iter()
                        .filter(|each| each.contains(link.as_str()))
                        .count(),
                    1,
                    "{link:?} is not said exactly once in {links:?}"
                );
            }
            let handed_over = error::Error::source(&crossed)
                .and_then(|below| below.downcast_ref::<Error>())
                .expect("the gateway's own error crosses whole");
            assert!(
                matches!(
                    handed_over,
                    Error::UnreadableTokenResponse { status: 200, .. }
                ),
                "{handed_over:?}"
            );
        }
    }

    // The loopback the browser is sent back to is this machine's own work, and
    // what stopped it is the operating system's answer: the port's word for
    // that is `Io`, with the kind kept, and not a verdict on credentials that
    // Storage was never asked about.
    #[test]
    fn a_loopback_that_will_not_run_reaches_the_port_as_a_local_failure() {
        let steps = [
            (RedirectStep::Bind, "could not listen for the redirect"),
            (RedirectStep::Port, "could not read the redirect port"),
            (RedirectStep::Accept, "could not accept the redirect"),
            (RedirectStep::Read, "could not read the redirect"),
        ];
        for (step, said) in steps {
            let error = Error::LoopbackRedirect {
                step,
                cause: io::Error::new(io::ErrorKind::AddrInUse, "Address already in use"),
            };
            // The step is what this layer knows; what the operating system
            // answered is the link under it, said once.
            assert_eq!(
                chain(&error),
                vec![
                    format!("authorization did not complete: {said}"),
                    "Address already in use".to_owned(),
                ],
            );
            let crossed = coffret_usecase::Error::from(error);
            let coffret_usecase::Error::Io { cause } = &crossed else {
                panic!("a loopback that will not run is this machine's own failure: {crossed:?}");
            };
            assert_eq!(cause.kind(), io::ErrorKind::AddrInUse);
            assert!(!crossed.is_retryable());
            assert_eq!(crossed.redacted(), "Io(kind=AddrInUse)");
        }
    }

    // A grant wider than the one permission asked for is no credential to work
    // from, whether the endpoint named what it granted or named nothing: the
    // person has to authorize again, which is what the port's
    // `Unauthenticated` says.
    #[test]
    fn a_grant_that_is_not_drive_file_alone_reaches_the_port_as_unauthenticated() {
        let grants = [
            Some(GrantedScopes::parse(&format!(
                "{DRIVE_FILE_SCOPE} https://www.googleapis.com/auth/drive"
            ))),
            None,
        ];
        for granted in grants {
            let error = Error::GrantNotDriveFileAlone { granted };
            // A refusal is worth nothing to its reader without the scope that
            // was expected of the grant.
            assert!(error.to_string().contains(DRIVE_FILE_SCOPE), "{error}");
            assert!(error::Error::source(&error).is_none());
            assert!(matches!(
                coffret_usecase::Error::from(error),
                coffret_usecase::Error::Unauthenticated { .. }
            ));
        }
    }

    // What these four have in common is only where they land: no credential
    // came of the flow, and no retry produces one.
    #[test]
    fn every_way_the_flow_can_end_without_a_grant_reaches_the_port_as_unauthenticated() {
        let endings = [
            (
                Error::ProviderRefusedAuthorization {
                    refusal: "access_denied".to_owned(),
                },
                "access_denied",
            ),
            (Error::RedirectWithoutState, "the state this flow sent"),
            (
                Error::RedirectTimedOut {
                    after: Duration::from_secs(300),
                },
                "within 300s",
            ),
            (Error::GrantWithoutRefreshToken, "no refresh token"),
        ];
        for (error, said) in endings {
            assert!(error.to_string().contains(said), "{error}");
            // What each carries is a fact this layer or the provider stated,
            // never a Rust error it observed.
            assert!(error::Error::source(&error).is_none(), "{error}");

            let crossed = coffret_usecase::Error::from(error);
            assert!(
                matches!(crossed, coffret_usecase::Error::Unauthenticated { .. }),
                "{crossed:?}"
            );
            assert!(!crossed.is_retryable(), "{crossed}");
        }
    }

    #[test]
    fn a_redirect_target_that_is_not_a_url_reaches_the_port_as_unauthenticated() {
        let target = ":99999999";
        let cause = url::Url::parse(&format!("http://127.0.0.1{target}"))
            .expect_err("no port is that large");
        let error = Error::MalformedRedirect {
            target: target.to_owned(),
            cause,
        };

        // What the browser asked for is quoted, so a target with whitespace or
        // control bytes in it is still readable in a log.
        assert!(
            error.to_string().contains(&format!("{target:?}")),
            "{error}"
        );
        assert!(error::Error::source(&error).is_some());
        assert!(matches!(
            coffret_usecase::Error::from(error),
            coffret_usecase::Error::Unauthenticated { .. }
        ));
    }

    // A client that cannot be built is not something a retry or a fresh
    // authorization would help with: this build asked for something the library
    // cannot do.
    #[test]
    fn a_client_that_cannot_be_built_reaches_the_port_as_unsupported() {
        // No pair of TLS versions is both at least 1.3 and at most 1.2, so the
        // builder refuses without a network being involved.
        let cause = reqwest::Client::builder()
            .min_tls_version(reqwest::tls::Version::TLS_1_3)
            .max_tls_version(reqwest::tls::Version::TLS_1_2)
            .build()
            .expect_err("no TLS version satisfies both bounds");
        let error = Error::HttpClient { cause };

        assert!(error::Error::source(&error).is_some());
        assert!(matches!(
            coffret_usecase::Error::from(error),
            coffret_usecase::Error::Unsupported { .. }
        ));
    }

    const FOLDER_NAME: &str = "coffret-0123456789abcdef";

    // A call that failed was classified where Drive's answer was read, and
    // whether trying again could help is what that classification carries. The
    // crossing has to leave it alone rather than name a verdict of its own.
    #[test]
    fn a_call_that_failed_reaches_the_port_as_what_it_was_classified_as() {
        let error = Error::AppFolderNotCreated {
            name: FOLDER_NAME.to_owned(),
            cause: AppFolderDefect::Call(coffret_usecase::Error::RateLimited {
                retry_after: None,
                detail: "the account is calling too often".to_owned(),
                source: None,
            }),
        };

        assert!(error::Error::source(&error).is_some());
        let crossed = coffret_usecase::Error::from(error);
        assert!(
            matches!(crossed, coffret_usecase::Error::RateLimited { .. }),
            "{crossed:?}"
        );
        assert!(crossed.is_retryable());
    }

    #[test]
    fn an_answer_naming_no_folder_reaches_the_port_as_a_malformed_response() {
        let error = Error::AppFolderNotCreated {
            name: FOLDER_NAME.to_owned(),
            cause: AppFolderDefect::Answer(json_error()),
        };

        assert!(error::Error::source(&error).is_some());
        let coffret_usecase::Error::MalformedResponse { detail, .. } =
            coffret_usecase::Error::from(error)
        else {
            panic!("an answer this build cannot read is a malformed response");
        };
        // The port's variant has nowhere to name a folder of its own, so what
        // this layer knew about it travels in the message — and in the error
        // itself, handed over whole beside it.
        assert!(detail.contains(FOLDER_NAME), "{detail}");
    }

    // Drive answered every page of the walk and never said the listing was
    // over. That is not an answer this build cannot read, and the port has its
    // own word for it now: the listing outran the pages this device reads.
    #[test]
    fn a_listing_that_never_ended_reaches_the_port_as_one_past_its_cap() {
        let error = Error::LibraryObjectUnreadable {
            folder_id: "1FoLdEr".to_owned(),
            name: "head-1.cfrt".to_owned(),
            cause: Box::new(AppFolderDefect::UnendingListing { pages: 1_000 }),
        };

        let crossed = coffret_usecase::Error::from(error);
        let coffret_usecase::Error::ListingPastCap { pages, .. } = &crossed else {
            panic!("a listing that never ended is not {crossed:?}");
        };
        assert_eq!(*pages, 1_000);
        assert!(!crossed.is_retryable());
        assert_eq!(crossed.redacted(), "Storage::ListingPastCap(pages=1000)");
        let handed_over = error::Error::source(&crossed)
            .and_then(|below| below.downcast_ref::<Error>())
            .expect("the gateway's own error crosses whole");
        assert!(
            matches!(handed_over, Error::LibraryObjectUnreadable { .. }),
            "{handed_over:?}"
        );
    }

    // A wrapper says which step of this layer's work refused and what it was
    // working on; the typed cause says what the layer below answered. A caller
    // printing `{error:#}` reads each of those once, all the way down through
    // the defect's own vocabulary to the format layer's refusal.
    #[test]
    fn an_unreadable_cache_reaches_a_caller_as_one_sentence_per_layer() {
        let error = Error::MalformedTokenCache {
            path: path(),
            cause: TokenCacheDefect::Sealed(coffret_format::Error::AuthenticationFailed),
        };

        assert_eq!(
            chain(&error),
            vec![
                "the token cache at \"/home/someone/.config/coffret/tokens.bin\" is unreadable"
                    .to_owned(),
                "the sealed form could not be opened".to_owned(),
                "message failed authentication".to_owned(),
            ],
        );
    }

    // What the layers beneath a wrapper said is rendered into the message of
    // the `io::Error` a local failure crosses in — and said once. A wrapper's
    // own line no longer repeats its cause, so a `to_string()` here would
    // strand the sentence that explains the refusal.
    #[test]
    fn a_cause_under_a_wrapper_crosses_the_port_inside_the_io_message_exactly_once() {
        let error = Error::LoopbackRedirect {
            step: RedirectStep::Bind,
            cause: io::Error::new(io::ErrorKind::AddrInUse, "Address already in use"),
        };

        let coffret_usecase::Error::Io { cause } = coffret_usecase::Error::from(error) else {
            panic!("a loopback that will not run is this machine's own failure");
        };
        let said = cause.to_string();
        assert_eq!(
            said,
            "authorization did not complete: could not listen for the redirect: \
             Address already in use",
        );
        assert_eq!(said.matches("Address already in use").count(), 1, "{said}");
    }

    // The same of a chain three links deep, whose middle link is this crate's
    // own defect vocabulary: every one of them crosses, in the order a person
    // reading `{error:#}` meets them.
    #[test]
    fn every_link_of_a_deeper_chain_crosses_the_port_in_the_order_it_is_read() {
        let error = Error::MalformedTokenCache {
            path: path(),
            cause: TokenCacheDefect::Sealed(coffret_format::Error::AuthenticationFailed),
        };
        let links = chain(&error);

        let coffret_usecase::Error::Io { cause } = coffret_usecase::Error::from(error) else {
            panic!("a cache this build cannot read is this machine's own failure");
        };
        assert_eq!(cause.to_string(), links.join(": "));
        assert!(
            cause.to_string().contains("message failed authentication"),
            "{cause}",
        );
    }

    // A foreign library's chain is not rendered into anything at the port: the
    // `detail` is this crate's own line, and the library's sentence and what
    // it keeps underneath — a client library hangs the request's URL, and so
    // the host somebody configured, off links of its own — are there for
    // whoever walks the chain, under the error the port carries as its
    // `source`.
    #[test]
    fn a_foreign_chain_crosses_the_port_as_links_rather_than_as_a_sentence() {
        // No pair of TLS versions is both at least 1.3 and at most 1.2, as
        // above.
        let cause = reqwest::Client::builder()
            .min_tls_version(reqwest::tls::Version::TLS_1_3)
            .max_tls_version(reqwest::tls::Version::TLS_1_2)
            .build()
            .expect_err("no TLS version satisfies both bounds");
        let said = cause.to_string();
        let beneath = error::Error::source(&cause)
            .expect("this library keeps what it was told on a link of its own")
            .to_string();
        assert!(
            !said.contains(&beneath),
            "the library's own sentence already spells what it hangs beneath it, \
             so this case would prove nothing: {said}",
        );

        let crossed = coffret_usecase::Error::from(Error::HttpClient { cause });
        let coffret_usecase::Error::Unsupported { detail, .. } = &crossed else {
            panic!("a client that cannot be built is something this build asked for");
        };
        assert_eq!(detail, "could not build an HTTP client");
        let links = chain(&crossed);
        assert!(links.contains(&said), "{links:?}");
        assert!(links.contains(&beneath), "{links:?}");
    }

    #[test]
    fn an_entropy_source_that_will_not_answer_reaches_the_port_as_a_local_failure() {
        let error = Error::EntropyUnavailable {
            cause: getrandom::Error::UNSUPPORTED,
        };

        assert!(error::Error::source(&error).is_some());
        assert!(matches!(
            coffret_usecase::Error::from(error),
            coffret_usecase::Error::Io { .. }
        ));
    }
}
