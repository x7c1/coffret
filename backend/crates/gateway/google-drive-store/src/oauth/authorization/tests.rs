//! What the flow asks for, what it will take back, and how it ends when
//! nothing it can keep ever arrives
//! (spec: SA-1, SA-2, SA-3, SA-4, SA-5, SA-6).

use coffret_format::{Purpose, PurposeKey};
use coffret_logging::testing::CapturedLogs;
use coffret_model::MasterKey;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tracing::Level;

use super::*;
use crate::http::{StubAnswer, StubTransport};

/// The account-wide grant: what a widened consent would carry beside the one
/// permission that was asked for.
const DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive";

/// Tokens shaped like the ones Google issues, so a search for either of them in
/// a refusal would find it if one were ever composed into a message.
const ACCESS_TOKEN: &str = "ya29.AnAccessToken";
const REFRESH_TOKEN: &str = "1//0gSecretRefreshToken";

/// The key a cache is sealed under here, derived as a device derives it.
fn cache_key() -> Arc<PurposeKey> {
    Arc::new(PurposeKey::derive(
        &MasterKey::from_bytes([0x3d; MasterKey::BYTE_LEN]),
        Purpose::TokenCache,
    ))
}

/// A successful token response, granting whatever `scope` says — or, where it
/// says nothing, naming no scope at all.
fn token_response(scope: Option<&str>) -> String {
    let granted = match scope {
        Some(scope) => format!(r#","scope":"{scope}""#),
        None => String::new(),
    };
    format!(
        r#"{{"access_token":"{ACCESS_TOKEN}","expires_in":3599,"refresh_token":"{REFRESH_TOKEN}"{granted}}}"#
    )
}

/// Runs the code exchange against an endpoint scripted to answer `body`.
///
/// The temporary directory comes back so that the cache it holds outlives the
/// call: what a refused exchange left behind is half of what is being asserted.
async fn exchange(body: &str) -> (tempfile::TempDir, TokenCache, Result<()>) {
    let directory = tempfile::tempdir().expect("a temporary directory must be available");
    let cache = TokenCache::new(directory.path().join("tokens.bin"), cache_key());
    let authorization = Authorization::new(
        StubTransport::new([StubAnswer::json(200, body)]),
        ClientCredentials::new("client-id"),
        cache.clone(),
    )
    .with_token_endpoint(TokenEndpoint::new("https://oauth2.example/token"));

    let pkce = PkceChallenge::generate().expect("entropy must be available");
    let outcome = authorization
        .exchange("the-code", "http://127.0.0.1:1234", &pkce)
        .await;

    (directory, cache, outcome)
}

/// Asserts that a response granting `scope` is refused and cached nothing
/// (spec: SA-4), in a refusal that names the grant and no token (spec: SA-5),
/// and that the refusal is recorded (spec: EL-1, EL-5).
async fn assert_refused(scope: Option<&str>) {
    let logs = CapturedLogs::capture();
    let (_directory, cache, outcome) = exchange(&token_response(scope)).await;

    // The endpoint answered 200, so nothing it does records anything: without
    // this event a person who authorized the wrong thing has the terminal and
    // nothing else to read afterwards.
    let event = logs.only(Level::WARN);
    assert_eq!(event.field("operation"), "authorize");
    match scope {
        // A `scope` field that names nothing is an empty set: an answer that
        // said what it granted and granted none. That is the case the field
        // below has to be told apart from, so both halves of the distinction
        // are pinned rather than only the one.
        Some(scope) if scope.split_whitespace().next().is_none() => {
            assert_eq!(event.field("granted"), "no scope at all");
        }
        // The scope set is the provider's own public identifier for what the
        // consent screen offered, so the record may name it in full.
        Some(scope) => {
            for named in scope.split_whitespace() {
                assert!(event.field("granted").contains(named), "{event}");
            }
        }
        // An answer that named no scope is not an answer that named an empty
        // one, and the record says which it was.
        None => assert_eq!(event.field("granted"), "the answer named no scope"),
    }
    // Everything else the grant arrived with is a bearer credential or a step
    // on the way to one, and none of it belongs in an event (spec: EL-1).
    logs.assert_free_of(&[
        ACCESS_TOKEN,
        REFRESH_TOKEN,
        "0gSecret",
        "the-code",
        "refresh_token",
        "tokens.bin",
    ]);

    let error = outcome.expect_err(&format!("{scope:?} must not be cached"));
    let Error::GrantNotDriveFileAlone { granted } = &error else {
        panic!("a grant that is not drive.file alone must be refused as such: {error}");
    };
    match (granted, scope) {
        // What was granted is named, so the person can go and look at the
        // consent they clicked through.
        (Some(granted), Some(scope)) => {
            for named in scope.split_whitespace() {
                assert!(granted.to_string().contains(named), "{granted}");
            }
        }
        (None, None) => {}
        _ => {
            panic!("the refusal must carry what the answer named, and nothing where it named none")
        }
    }

    let message = error.to_string();
    for secret in [ACCESS_TOKEN, REFRESH_TOKEN] {
        assert!(
            !message.contains(secret),
            "a token must never appear in a refusal: {message}"
        );
    }

    assert_eq!(
        cache.load().expect("the cache must be readable"),
        None,
        "a refused grant must leave the cache empty"
    );
}

// SA-3: one permission is asked for and no other. SA-1 rides along on the same
// URL — the challenge method is `S256`, and the verifier is not on it.
#[test]
fn the_authorization_url_asks_for_drive_file_and_nothing_else() {
    let authorization = Authorization::new(
        Arc::new(crate::http::ReqwestTransport::with_default_client().unwrap()),
        ClientCredentials::new("client-id"),
        TokenCache::new("/nonexistent/tokens.bin", cache_key()),
    );
    let pkce = PkceChallenge::generate().unwrap();
    let url = authorization.authorization_url("http://127.0.0.1:1234", &pkce, "s3cr3t");
    let parsed = url::Url::parse(&url).expect("the authorization URL must be a URL");

    let scopes: Vec<_> = parsed
        .query_pairs()
        .filter(|(key, _)| key == "scope")
        .map(|(_, value)| value.into_owned())
        .collect();
    assert_eq!(scopes, [DRIVE_FILE_SCOPE]);

    assert!(url.contains("code_challenge_method=S256"));
    assert!(
        !url.contains(pkce.verifier()),
        "the verifier must never leave the process"
    );
}

// SA-4, and the case a containment test waves through: `drive.file` is in the
// answer, and so is a grant over every file in the account. Caching the refresh
// token behind it would make it a bearer credential for the whole account.
#[tokio::test]
async fn refuses_a_grant_wider_than_drive_file() {
    assert_refused(Some(&format!("{DRIVE_FILE_SCOPE} {DRIVE_SCOPE}"))).await;
    assert_refused(Some(&format!("{DRIVE_FILE_SCOPE} openid"))).await;
}

// SA-4 over a different set: what came back is not the permission that was
// asked for at all.
#[tokio::test]
async fn refuses_a_grant_without_drive_file() {
    assert_refused(Some(DRIVE_SCOPE)).await;
    assert_refused(Some("")).await;
}

// SA-4's sub-bullet. An answer that names no scope verifies nothing, and
// "identical to what was requested" is an assumption rather than a check.
#[tokio::test]
async fn refuses_a_grant_that_names_no_scope() {
    assert_refused(None).await;
}

// SA-4 from the other side: the grant that is exactly the one asked for is
// accepted however it is spelled, and it is the one thing that reaches the
// cache.
#[tokio::test]
async fn accepts_drive_file_however_it_is_spelled_out() {
    let spellings = [
        DRIVE_FILE_SCOPE.to_owned(),
        format!("{DRIVE_FILE_SCOPE} {DRIVE_FILE_SCOPE}"),
        format!("  {DRIVE_FILE_SCOPE}   {DRIVE_FILE_SCOPE}  "),
    ];
    for spelling in spellings {
        let (_directory, cache, outcome) = exchange(&token_response(Some(&spelling))).await;
        outcome.unwrap_or_else(|error| panic!("{spelling:?} is the grant asked for: {error}"));

        assert_eq!(
            cache.load().expect("the cache must be readable"),
            Some(StoredTokens {
                refresh_token: REFRESH_TOKEN.to_owned(),
            }),
            "{spelling:?}"
        );
    }
}

// The answer a person is likeliest to meet: they authorized once before, the
// grant was never revoked, and the provider hands back an access token alone.
// It is the grant that was asked for, and there is still nothing to cache.
#[tokio::test]
async fn refuses_a_grant_that_carries_no_refresh_token() {
    let body = format!(
        r#"{{"access_token":"{ACCESS_TOKEN}","expires_in":3599,"scope":"{DRIVE_FILE_SCOPE}"}}"#
    );
    let logs = CapturedLogs::capture();
    let (_directory, cache, outcome) = exchange(&body).await;

    let error = outcome.expect_err("an access token alone is nothing to cache");
    assert!(
        matches!(error, Error::GrantWithoutRefreshToken),
        "{error:?}"
    );
    assert_eq!(
        cache.load().expect("the cache must be readable"),
        None,
        "a grant with nothing durable in it must leave the cache empty"
    );

    // The endpoint answered 200 here too, so this refusal is as invisible as
    // the other one unless the flow records it itself.
    let event = logs.only(Level::WARN);
    assert_eq!(event.field("operation"), "authorize");
    assert!(event.message().contains("no refresh token"), "{event}");
    logs.assert_free_of(&[ACCESS_TOKEN, "ya29.", "the-code", "tokens.bin"]);
}

// SA-2 from the flow's end rather than the parser's: something aimed a request
// at the loopback port without the state this run sent, which is the CSRF check
// failing. It is refused by that name, and nothing about it is kept.
#[tokio::test]
async fn a_redirect_without_the_state_this_flow_sent_caches_nothing() {
    let directory = tempfile::tempdir().expect("a temporary directory must be available");
    let cache = TokenCache::new(directory.path().join("tokens.bin"), cache_key());
    // Nothing is scripted: a redirect that fails this check must never be
    // traded for a token, and a call made anyway panics rather than passing.
    let authorization = Authorization::new(
        StubTransport::new([]),
        ClientCredentials::new("client-id"),
        cache.clone(),
    )
    .with_token_endpoint(TokenEndpoint::new("https://oauth2.example/token"));

    let outcome = authorization
        .run(|url| {
            let url = url.to_owned();
            tokio::spawn(async move { knock(&url, "&state=elsewhere").await });
        })
        .await;

    assert!(
        matches!(outcome, Err(Error::RedirectWithoutState)),
        "{outcome:?}"
    );
    assert_eq!(
        cache.load().expect("the cache must be readable"),
        None,
        "a redirect that is not this flow's own must leave the cache empty"
    );
}

// The other way a browser never brings the code back: nobody ever arrives. What
// the flow knows is how long it waited, so that is what the refusal carries
// rather than a sentence composed around it.
#[tokio::test(start_paused = true)]
async fn a_redirect_that_never_arrives_is_refused_by_how_long_it_was_waited_for() {
    let directory = tempfile::tempdir().expect("a temporary directory must be available");
    let cache = TokenCache::new(directory.path().join("tokens.bin"), cache_key());
    let authorization = Authorization::new(
        StubTransport::new([]),
        ClientCredentials::new("client-id"),
        cache.clone(),
    );

    // Time is paused, so the wait costs the test nothing and still is the wait
    // the flow was built with.
    let outcome = authorization.run(|_| {}).await;

    let Err(Error::RedirectTimedOut { after }) = &outcome else {
        panic!("a redirect that never came must be refused as such: {outcome:?}");
    };
    assert_eq!(*after, REDIRECT_TIMEOUT);
    assert_eq!(cache.load().expect("the cache must be readable"), None);
}

/// Knocks on the loopback the authorization URL points back at, the way a
/// browser returning from the consent screen does.
///
/// `extra` is appended to the query, so a case decides what the arrival carries
/// beside its code.
async fn knock(authorization_url: &str, extra: &str) {
    let parsed = url::Url::parse(authorization_url).expect("the authorization URL must be a URL");
    let redirect = parsed
        .query_pairs()
        .find(|(key, _)| key == "redirect_uri")
        .map(|(_, value)| value.into_owned())
        .expect("the flow says where it is to be redirected back to");

    let back = url::Url::parse(&redirect).expect("the redirect target must be a URL");
    let address = format!(
        "{}:{}",
        back.host_str().expect("the loopback has a host"),
        back.port().expect("the loopback has a port"),
    );

    let mut stream = TcpStream::connect(address)
        .await
        .expect("the flow must be listening");
    let request = format!(
        "GET /?code=4%2Fabc{extra} HTTP/1.1\r\nhost: 127.0.0.1\r\nconnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .await
        .expect("the request must be writable");
}
