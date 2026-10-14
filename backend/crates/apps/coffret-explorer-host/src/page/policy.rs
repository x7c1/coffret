use std::sync::OnceLock;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use sha2::{Digest, Sha256};

/// The `Content-Security-Policy` every page and asset is served with.
///
/// Everything from the host's own origin and nothing from anywhere else; no
/// plugin, no `<base>`, no framing by any page (spec: LA-16), no form
/// submitted anywhere, and no `eval`. Images may also be `blob:` URLs, which
/// is how the explorer shows a file it fetched. The one exception is the
/// stylesheet the explorer's `index.html` carries inline: each `<style>` in it
/// is admitted by its own hash and no other inline style is, so that the
/// exceptions are exactly what this build needs.
pub(crate) fn content_security_policy() -> &'static str {
    static POLICY: OnceLock<String> = OnceLock::new();
    POLICY.get_or_init(|| policy_for(index()))
}

#[cfg(feature = "embed-web")]
fn index() -> Option<&'static [u8]> {
    super::BUILT
        .get_file(super::INDEX)
        .map(include_dir::File::contents)
}

#[cfg(not(feature = "embed-web"))]
fn index() -> Option<&'static [u8]> {
    None
}

/// The policy for a build whose `index.html` is `index`.
pub(super) fn policy_for(index: Option<&[u8]>) -> String {
    let mut style = String::from("style-src 'self'");
    let index = index.map(String::from_utf8_lossy).unwrap_or_default();
    for inline in inline_styles(&index) {
        let digest = Sha256::digest(inline.as_bytes());
        style.push_str(&format!(" 'sha256-{}'", STANDARD.encode(digest)));
    }
    [
        "default-src 'self'",
        "script-src 'self'",
        &style,
        "img-src 'self' blob:",
        "connect-src 'self'",
        "font-src 'self'",
        "object-src 'none'",
        "base-uri 'none'",
        "frame-ancestors 'none'",
        "form-action 'none'",
    ]
    .join("; ")
}

/// The text of every `<style>` element in `html`, exactly as it stands between
/// its tags, which is what a browser hashes.
fn inline_styles(html: &str) -> Vec<&str> {
    let mut styles = Vec::new();
    let mut rest = html;
    while let Some(open) = rest.find("<style") {
        let after = &rest[open..];
        let Some(start) = after.find('>') else { break };
        let body = &after[start + 1..];
        let Some(end) = body.find("</style>") else {
            break;
        };
        styles.push(&body[..end]);
        rest = &body[end..];
    }
    styles
}
