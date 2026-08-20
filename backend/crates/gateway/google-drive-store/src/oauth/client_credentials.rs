/// Which OAuth client this gateway authorizes as.
///
/// An installed application's "secret" is not one — it ships with the binary
/// and Google says as much — which is exactly why the flow is PKCE-protected
/// rather than resting on it. It is optional here because a client registered
/// without one is equally usable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientCredentials {
    client_id: String,
    client_secret: Option<String>,
}

impl ClientCredentials {
    /// Takes the client id of a registered installed application.
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: None,
        }
    }

    /// Adds the client secret, for a client registered with one.
    pub fn with_client_secret(mut self, client_secret: impl Into<String>) -> Self {
        self.client_secret = Some(client_secret.into());
        self
    }

    /// The client id.
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// The client secret, if the client has one.
    pub fn client_secret(&self) -> Option<&str> {
        self.client_secret.as_deref()
    }
}
