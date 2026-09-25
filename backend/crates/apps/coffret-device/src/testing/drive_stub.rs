use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use coffret_usecase::ByteStream;
use google_drive_store::http::{HttpRequest, HttpResponse, Method, RequestBody, TransportError};
use google_drive_store::{HttpTransport, DRIVE_API};

/// Where the token exchange goes, as the gateway addresses it.
const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";

/// The one permission a grant may carry (spec: SA-3).
const DRIVE_FILE_SCOPE: &str = "https://www.googleapis.com/auth/drive.file";

/// What every access token this stub mints starts with, before the grant's
/// number.
const ACCESS_PREFIX: &str = "ya29.stub-access-";

/// What every refresh token this stub mints starts with, before the grant's
/// number.
const REFRESH_PREFIX: &str = "1//stub-refresh-";

/// The id Drive mints for the one folder a case creates.
pub(crate) const CREATED_FOLDER_ID: &str = "stub-folder-1";

/// A Drive that answers the calls putting a Library on this device makes, and
/// nothing else.
///
/// Four of them, and each is answered the way Drive answers it: the token
/// exchange at the end of a consent, a folder created under a parent, a folder's
/// name read back by id, and a listing of a folder by name. It answers by what a
/// call is addressed at rather than from a script, because what a case over the
/// device layer is about is the flow's order of calls and what it makes of the
/// answers — a script would be a second copy of that order, written by hand.
///
/// It is a transport and not a store: it implements the gateway's
/// [`HttpTransport`] the way the shipping `ReqwestTransport` does, so the
/// gateway's own request building and answer reading run exactly as they ship.
/// What a real Drive decides about any of it is the gateway's conformance
/// suite's business, and that runs against a real account.
pub(crate) struct DriveStub {
    /// The folders Drive holds, by id: those a case put there, and those the
    /// flow created — each with the one grant it is visible to, where it is
    /// visible to one grant and not to every grant.
    ///
    /// Every consent answered here is a grant of its own, numbered from 1 in
    /// the order they were given, which is what lets one stub stand for two
    /// accounts: a `drive.file` grant sees only the files its own account's
    /// application created (spec: SA-3).
    folders: Mutex<BTreeMap<String, (String, Option<u32>)>>,
    /// How many consents have been traded for a grant.
    grants: Mutex<u32>,
    /// Whether a listing of a folder finds what it was asked for by name.
    holds_the_library: bool,
    /// What each call was, as its method and the URL it was addressed at.
    calls: Mutex<Vec<(&'static str, String)>>,
}

impl DriveStub {
    /// A Drive holding no folders, whose listings find nothing.
    pub(crate) fn empty() -> Arc<Self> {
        Arc::new(Self {
            folders: Mutex::new(BTreeMap::new()),
            grants: Mutex::new(0),
            holds_the_library: false,
            calls: Mutex::new(Vec::new()),
        })
    }

    /// A Drive holding one folder called `name`, whose listings find what they
    /// ask for — the Library another device created and has committed into.
    pub(crate) fn holding(id: &str, name: &str) -> Arc<Self> {
        Arc::new(Self {
            folders: Mutex::new(BTreeMap::from([(id.to_owned(), (name.to_owned(), None))])),
            grants: Mutex::new(0),
            holds_the_library: true,
            calls: Mutex::new(Vec::new()),
        })
    }

    /// Puts a folder called `name` under `id` that only the `grant`th consent
    /// answered here can see — the folder of one account among several.
    pub(crate) fn add_folder_for(&self, id: &str, name: &str, grant: u32) {
        self.folders
            .lock()
            .expect("no case panics while holding this")
            .insert(id.to_owned(), (name.to_owned(), Some(grant)));
    }

    /// How many consents have been traded for a grant so far.
    pub(crate) fn consents(&self) -> u32 {
        *self
            .grants
            .lock()
            .expect("no case panics while holding this")
    }

    /// The name of the folder Drive holds under `id`, if it holds one.
    pub(crate) fn folder_named(&self, id: &str) -> Option<String> {
        self.folders
            .lock()
            .expect("no case panics while holding this")
            .get(id)
            .map(|(name, _)| name.clone())
    }

    /// Each call made, as its method and what it was addressed at, in order.
    pub(crate) fn calls(&self) -> Vec<(&'static str, String)> {
        self.calls
            .lock()
            .expect("no case panics while holding this")
            .clone()
    }

    /// Each call made, by which of the four questions it was, in order:
    /// `token`, `create`, `name` or `list`.
    pub(crate) fn asked(&self) -> Vec<&'static str> {
        let files = format!("{DRIVE_API}/files");
        self.calls()
            .into_iter()
            .map(|(method, url)| match method {
                _ if url.starts_with(TOKEN_ENDPOINT) => "token",
                "POST" => "create",
                _ if url.starts_with(&format!("{files}/")) => "name",
                _ => "list",
            })
            .collect()
    }

    fn answer(&self, method: &Method, url: &str, body: &[u8], grant: Option<u32>) -> (u16, String) {
        let files = format!("{DRIVE_API}/files");
        match method {
            // What a consent is traded for: a refresh token of its own, and
            // exactly the scope that was asked for, which is what the flow
            // checks before it caches anything (spec: SA-4, SA-6). A refresh
            // mints an access token for the grant whose refresh token it
            // carries.
            Method::Post if url.starts_with(TOKEN_ENDPOINT) => {
                let grant = self.grant_of_exchange(body);
                (
                    200,
                    format!(
                        r#"{{"access_token":"{ACCESS_PREFIX}{grant}","expires_in":3599,"refresh_token":"{REFRESH_PREFIX}{grant}","scope":"{DRIVE_FILE_SCOPE}"}}"#
                    ),
                )
            }
            Method::Post if url.starts_with(&format!("{files}?")) => {
                let metadata: serde_json::Value = serde_json::from_slice(body)
                    .expect("a folder is created from a JSON description of it");
                let name = metadata["name"]
                    .as_str()
                    .expect("a folder is created with a name")
                    .to_owned();
                self.folders
                    .lock()
                    .expect("no case panics while holding this")
                    .insert(CREATED_FOLDER_ID.to_owned(), (name, None));
                (200, format!(r#"{{"id":"{CREATED_FOLDER_ID}"}}"#))
            }
            Method::Get if url.starts_with(&format!("{files}?")) => {
                if self.holds_the_library {
                    (200, r#"{"files":[{"id":"stub-object-1"}]}"#.to_owned())
                } else {
                    (200, r#"{"files":[]}"#.to_owned())
                }
            }
            Method::Get if url.starts_with(&format!("{files}/")) => {
                let id = url[files.len() + 1..].split('?').next().unwrap_or_default();
                let visible = self
                    .folders
                    .lock()
                    .expect("no case panics while holding this")
                    .get(id)
                    .filter(|(_, owner)| owner.is_none() || *owner == grant)
                    .map(|(name, _)| name.clone());
                match visible {
                    Some(name) => (200, serde_json::json!({ "name": name }).to_string()),
                    None => (
                        404,
                        r#"{"error":{"code":404,"message":"File not found."}}"#.to_owned(),
                    ),
                }
            }
            _ => panic!("the flow made a call this stub does not answer: {url}"),
        }
    }
}

impl DriveStub {
    /// Which grant a token exchange is for: a new one for a code a consent
    /// handed over, the one it names for a refresh.
    fn grant_of_exchange(&self, body: &[u8]) -> u32 {
        let form: BTreeMap<String, String> = url::form_urlencoded::parse(body)
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        match form.get("grant_type").map(String::as_str) {
            Some("refresh_token") => form
                .get("refresh_token")
                .and_then(|token| token.strip_prefix(REFRESH_PREFIX))
                .and_then(|grant| grant.parse().ok())
                .expect("a refresh carries a refresh token this stub minted"),
            _ => {
                let mut grants = self
                    .grants
                    .lock()
                    .expect("no case panics while holding this");
                *grants += 1;
                *grants
            }
        }
    }
}

#[async_trait]
impl HttpTransport for DriveStub {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
        let body = match request.body {
            RequestBody::Empty => Vec::new(),
            RequestBody::Bytes(bytes) => bytes,
            RequestBody::Stream(_) => panic!("putting a Library on a device uploads nothing"),
        };
        let method = match request.method {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        };
        self.calls
            .lock()
            .expect("no case panics while holding this")
            .push((method, request.url.clone()));

        let grant = request
            .headers
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("authorization"))
            .and_then(|(_, value)| value.strip_prefix(&format!("Bearer {ACCESS_PREFIX}")))
            .and_then(|grant| grant.parse().ok());
        let (status, answer) = self.answer(&request.method, &request.url, &body, grant);
        Ok(HttpResponse::new(
            status,
            vec![("content-type".to_owned(), "application/json".to_owned())],
            ByteStream::from(answer.into_bytes()),
        ))
    }
}

/// What a person at a browser does with a consent URL: grants it, and is sent
/// back to the loopback redirect the URL names, carrying a code and the state.
///
/// On a thread of its own, because the flow calls this before it waits for the
/// redirect and must get the call back to go on to waiting. The connection is
/// queued by the listener the flow bound before it built the URL, so nothing
/// here races the wait.
pub(crate) fn consent(url: &str) {
    let parsed = url::Url::parse(url).expect("a consent URL is a URL");
    let param = |name: &str| {
        parsed
            .query_pairs()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
            .unwrap_or_else(|| panic!("a consent URL carries {name}"))
    };
    let redirect = url::Url::parse(&param("redirect_uri")).expect("a redirect URI is a URL");
    let state = param("state");
    let address = format!(
        "{}:{}",
        redirect.host_str().expect("the redirect names a host"),
        redirect
            .port()
            .expect("the redirect names the port it was bound at"),
    );

    std::thread::spawn(move || {
        let mut stream =
            TcpStream::connect(&address).expect("the flow is listening for the redirect");
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("code", "stub-code")
            .append_pair("state", &state)
            .finish();
        stream
            .write_all(format!("GET /?{query} HTTP/1.1\r\nHost: {address}\r\n\r\n").as_bytes())
            .expect("the redirect can be sent");
        // The completion page, which a browser would show; read so the flow's
        // answer is not written into a connection nobody reads.
        let mut page = Vec::new();
        let _ = stream.read_to_end(&mut page);
    });
}
