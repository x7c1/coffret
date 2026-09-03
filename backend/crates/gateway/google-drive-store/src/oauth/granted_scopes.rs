use std::collections::BTreeSet;
use std::fmt;

use crate::oauth::token_endpoint::DRIVE_FILE_SCOPE;

/// What a token endpoint said it granted.
///
/// RFC 6749 §3.3 makes the `scope` field a space-delimited, case-sensitive
/// list, so reading one is splitting on whitespace and keeping what is left:
/// repeated or surrounding spaces are delimiters rather than scopes, and naming
/// the same scope twice grants it once. What comes out is a set, which is the
/// shape the one question asked of a grant here — is it
/// [`DRIVE_FILE_SCOPE`] and nothing else? — actually has an answer in.
///
/// Scopes are not secrets: what the endpoint granted is what the person saw on
/// the consent screen, and it is the one part of a token response worth
/// repeating back to them. The tokens beside it in that response are secrets,
/// and none of them are here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrantedScopes {
    scopes: BTreeSet<String>,
}

impl GrantedScopes {
    /// Reads the `scope` field of a token response as the set it denotes.
    ///
    /// Splitting on whitespace is a shade more liberal than the single space
    /// §3.3 names, and it can widen no grant: whatever stands between two
    /// scopes, they are still two members and the set is still refused. What
    /// the liberty buys is that a lone `drive.file` padded with a stray tab or
    /// newline is read as the grant it spells rather than as an unknown scope.
    pub fn parse(scope: &str) -> Self {
        Self {
            scopes: scope.split_whitespace().map(str::to_owned).collect(),
        }
    }

    /// Whether what was granted is [`DRIVE_FILE_SCOPE`] and nothing besides.
    ///
    /// Deliberately not "does the grant carry `drive.file`": a grant that
    /// carries it *alongside* `drive` reaches the whole account, and a
    /// containment test would wave that through. Only the exact set is the
    /// permission coffret asked for.
    pub fn is_drive_file_alone(&self) -> bool {
        self.scopes.len() == 1 && self.scopes.contains(DRIVE_FILE_SCOPE)
    }
}

impl fmt::Display for GrantedScopes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.scopes.is_empty() {
            return f.write_str("no scope at all");
        }
        for (position, scope) in self.scopes.iter().enumerate() {
            if position > 0 {
                f.write_str(", ")?;
            }
            f.write_str(scope)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The account-wide grant, which is what a widened grant would carry.
    const DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive";

    #[test]
    fn drive_file_alone_is_the_grant_that_was_asked_for() {
        assert!(GrantedScopes::parse(DRIVE_FILE_SCOPE).is_drive_file_alone());
    }

    // Repeated delimiters and surrounding whitespace are punctuation, and a
    // scope named twice is the same one scope: all of these are the same set.
    #[test]
    fn repeats_and_extra_whitespace_name_the_same_set() {
        let spellings = [
            format!("{DRIVE_FILE_SCOPE} {DRIVE_FILE_SCOPE}"),
            format!("  {DRIVE_FILE_SCOPE}   {DRIVE_FILE_SCOPE}  "),
            format!("\t{DRIVE_FILE_SCOPE}\n"),
        ];
        for spelling in spellings {
            let granted = GrantedScopes::parse(&spelling);
            assert_eq!(
                granted,
                GrantedScopes::parse(DRIVE_FILE_SCOPE),
                "{spelling:?}"
            );
            assert!(granted.is_drive_file_alone(), "{spelling:?}");
        }
    }

    // The case the containment test used to miss: `drive.file` is in there, and
    // so is a grant over every file in the account.
    #[test]
    fn anything_granted_beside_drive_file_is_a_wider_grant() {
        let wider = [
            format!("{DRIVE_FILE_SCOPE} {DRIVE_SCOPE}"),
            format!("{DRIVE_SCOPE} {DRIVE_FILE_SCOPE}"),
            format!("{DRIVE_FILE_SCOPE} openid"),
            // The liberty taken over the single space §3.3 names buys tolerance
            // for padding and nothing else: however a second scope is delimited,
            // it is still a second scope.
            format!("{DRIVE_FILE_SCOPE}\t{DRIVE_SCOPE}\n"),
        ];
        for scope in wider {
            assert!(
                !GrantedScopes::parse(&scope).is_drive_file_alone(),
                "{scope:?}"
            );
        }
    }

    #[test]
    fn a_grant_that_does_not_carry_drive_file_is_not_it_either() {
        for scope in [DRIVE_SCOPE, "openid", ""] {
            assert!(
                !GrantedScopes::parse(scope).is_drive_file_alone(),
                "{scope:?}"
            );
        }
    }

    // RFC 6749 §3.3 says scope values are case-sensitive, so a lookalike in
    // another case is another scope rather than this one.
    #[test]
    fn the_scope_is_matched_case_sensitively() {
        assert!(!GrantedScopes::parse(&DRIVE_FILE_SCOPE.to_uppercase()).is_drive_file_alone());
    }

    // What is displayed is what the person can be shown, so it has to name
    // every scope — and say so plainly when there were none.
    #[test]
    fn what_was_granted_is_named_in_full() {
        let granted = GrantedScopes::parse(&format!("{DRIVE_FILE_SCOPE} {DRIVE_SCOPE}"));
        let shown = granted.to_string();
        assert!(shown.contains(DRIVE_FILE_SCOPE), "{shown}");
        assert!(shown.contains(DRIVE_SCOPE), "{shown}");

        assert_eq!(GrantedScopes::parse("   ").to_string(), "no scope at all");
    }
}
