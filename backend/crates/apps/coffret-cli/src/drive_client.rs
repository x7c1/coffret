//! Which OAuth client the Drive flows authorize as.
//!
//! Both `init` and `join` take the same two flags and fall back to the same two
//! environment variables, which is why they are here rather than in either of
//! them.

use anyhow::Context;

/// Where the OAuth client id comes from when it is not typed.
///
/// There is no client id built into this binary. Registering one is the
/// account owner's to do, and a shared one would put every user of coffret in
/// the same consent screen quota.
pub const CLIENT_ID: &str = "COFFRET_DRIVE_CLIENT_ID";

/// Where the client secret comes from when it is not typed.
///
/// A desktop client registered with a secret cannot exchange its
/// authorization code without it, so the secret follows the client id: typed,
/// or taken from the environment the id was taken from.
pub const CLIENT_SECRET: &str = "COFFRET_DRIVE_CLIENT_SECRET";

/// Which OAuth client to authorize as, from the flags or the environment.
pub fn credentials(
    client_id: &Option<String>,
    client_secret: &Option<String>,
) -> anyhow::Result<(String, Option<String>)> {
    let client_id = match client_id {
        Some(client_id) => client_id.clone(),
        None => std::env::var(CLIENT_ID)
            .with_context(|| format!("--client-id was not given and {CLIENT_ID} is not set"))?,
    };
    let client_secret = client_secret
        .clone()
        .or_else(|| std::env::var(CLIENT_SECRET).ok());

    Ok((client_id, client_secret))
}
