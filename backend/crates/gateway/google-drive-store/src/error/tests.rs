//! What each [`Error`] says, and what it becomes at the port.

use std::error;

use coffret_model::Redacted;

use super::*;
use crate::oauth::DRIVE_FILE_SCOPE;
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

// The `Io` mapping in `into_port.rs` is chosen deliberately and explained
// at length, and what it costs is that the port says "local transfer failed"
// about a credential store — so the choice is pinned here rather than only
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
    let cause =
        url::Url::parse(&format!("http://127.0.0.1{target}")).expect_err("no port is that large");
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
