use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::error::{Error, Result};

/// The one-time secret that binds an authorization code to this exact request.
///
/// The redirect comes back over plain loopback HTTP, where any other process on
/// the machine could be listening for it. PKCE is what makes intercepting the
/// code useless: the token exchange also has to present the verifier, which
/// never left this process, and only its hash was ever sent to Google.
pub struct PkceChallenge {
    verifier: String,
    challenge: String,
}

/// How the challenge is derived from the verifier.
///
/// The plain method exists and is worthless here — it would put the secret in
/// the very request the challenge is supposed to protect.
pub const CHALLENGE_METHOD: &str = "S256";

/// How many random bytes the verifier is drawn from.
///
/// 32 bytes is the most the specification's length limit allows once
/// base64url-encoded, and there is no reason to draw fewer.
const VERIFIER_BYTES: usize = 32;

impl PkceChallenge {
    /// Draws a fresh verifier and derives its challenge.
    pub fn generate() -> Result<Self> {
        let mut entropy = [0u8; VERIFIER_BYTES];
        getrandom::fill(&mut entropy).map_err(|error| Error::EntropyUnavailable {
            detail: error.to_string(),
        })?;

        let verifier = URL_SAFE_NO_PAD.encode(entropy);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        Ok(Self {
            verifier,
            challenge,
        })
    }

    /// The secret, sent only with the token exchange.
    pub fn verifier(&self) -> &str {
        &self.verifier
    }

    /// The hash of the secret, sent with the authorization request.
    pub fn challenge(&self) -> &str {
        &self.challenge
    }
}

/// Draws an opaque random token, for the `state` parameter.
///
/// It is what lets the redirect handler tell its own callback from one another
/// page on the machine aimed at the same loopback port.
pub fn random_token() -> Result<String> {
    let mut entropy = [0u8; VERIFIER_BYTES];
    getrandom::fill(&mut entropy).map_err(|error| Error::EntropyUnavailable {
        detail: error.to_string(),
    })?;

    Ok(URL_SAFE_NO_PAD.encode(entropy))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_challenge_is_the_hash_of_the_verifier_and_not_the_verifier() {
        let pkce = PkceChallenge::generate().expect("entropy must be available");

        assert_ne!(pkce.verifier(), pkce.challenge());
        assert_eq!(
            pkce.challenge(),
            URL_SAFE_NO_PAD.encode(Sha256::digest(pkce.verifier().as_bytes()))
        );
    }

    #[test]
    fn every_authorization_draws_its_own_secret() {
        let first = PkceChallenge::generate().expect("entropy must be available");
        let second = PkceChallenge::generate().expect("entropy must be available");

        assert_ne!(first.verifier(), second.verifier());
    }
}
