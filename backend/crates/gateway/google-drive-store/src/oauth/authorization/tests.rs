use coffret_format::{Purpose, PurposeKey};
use coffret_model::MasterKey;

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

/// Asserts that a response granting `scope` is refused and cached nothing.
async fn assert_refused(scope: Option<&str>) {
    let (_directory, cache, outcome) = exchange(&token_response(scope)).await;

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

// The case a containment test waves through: `drive.file` is in the answer, and
// so is a grant over every file in the account. Caching the refresh token
// behind it would make it a bearer credential for the whole account.
#[tokio::test]
async fn refuses_a_grant_wider_than_drive_file() {
    assert_refused(Some(&format!("{DRIVE_FILE_SCOPE} {DRIVE_SCOPE}"))).await;
    assert_refused(Some(&format!("{DRIVE_FILE_SCOPE} openid"))).await;
}

#[tokio::test]
async fn refuses_a_grant_without_drive_file() {
    assert_refused(Some(DRIVE_SCOPE)).await;
    assert_refused(Some("")).await;
}

// An answer that names no scope verifies nothing, and "identical to what was
// requested" is an assumption rather than a check.
#[tokio::test]
async fn refuses_a_grant_that_names_no_scope() {
    assert_refused(None).await;
}

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
