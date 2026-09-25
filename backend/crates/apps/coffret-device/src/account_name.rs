//! What a person calls a Storage account on this device.

use std::fmt;

use crate::error::{Error, Result};

/// The device-local account name: what the person calls one Storage account on
/// this device (spec: SA-8).
///
/// coffret cannot tell accounts apart by what it was granted — the one
/// permission it asks for names no account (spec: SA-3) — so the person does,
/// by naming each one. Like a device-local Library name it is theirs, is never
/// written to Storage, and never reaches a diagnostic event (spec: EL-1): it is
/// the directory the account's grant is kept in, and it is bound into every
/// envelope that opens that grant (spec: SA-9).
///
/// The rule for one is here and nowhere else: one to [`MAX_LEN`](Self::MAX_LEN)
/// characters, each an ASCII letter, a digit, `-` or `_`. That is a plain
/// directory name on every filesystem a device keeps state on — no separator,
/// nothing a case-folding volume or a shell reads twice, and no `.`, so no name
/// can be a directory an account is being built in. [`DEFAULT`](Self::DEFAULT)
/// passes it, being the name an account the person leaves unnamed is given.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct AccountName(String);

impl AccountName {
    /// What an account the person leaves unnamed is called (spec: SA-8).
    pub(crate) const DEFAULT: &'static str = "default";

    /// The longest name an account may have, in characters.
    pub(crate) const MAX_LEN: usize = 64;

    /// The name `name` spells, or the refusal that says what a name may be.
    pub(crate) fn parse(name: &str) -> Result<Self> {
        let plain = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
        if name.is_empty() || name.len() > Self::MAX_LEN || !name.chars().all(plain) {
            return Err(Error::InvalidAccountName {
                name: name.to_owned(),
            });
        }
        Ok(Self(name.to_owned()))
    }

    /// The name an account the person leaves unnamed is given.
    pub(crate) fn default_name() -> Self {
        Self(Self::DEFAULT.to_owned())
    }

    /// The name, as the person gave it.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AccountName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // SA-8: the name an unnamed account is given passes the rule every name is
    // held to, so every envelope has a name to be bound to.
    #[test]
    fn the_default_name_is_a_name() {
        assert_eq!(
            AccountName::parse(AccountName::DEFAULT)
                .expect("the default is a name")
                .as_str(),
            "default"
        );
        assert_eq!(AccountName::default_name().as_str(), AccountName::DEFAULT);
    }

    // A plain directory name, and nothing else.
    #[test]
    fn only_a_plain_directory_name_is_an_account_name() {
        for name in ["work", "Home-2", "a_b", "x", &"a".repeat(64)] {
            assert!(AccountName::parse(name).is_ok(), "{name:?} is a name");
        }
        for name in [
            "",
            ".",
            "..",
            "work/home",
            "work\\home",
            "work.old",
            ".hidden",
            "with space",
            "tab\there",
            "仕事",
            &"a".repeat(65),
        ] {
            assert!(
                matches!(
                    AccountName::parse(name),
                    Err(Error::InvalidAccountName { .. })
                ),
                "{name:?} is not a name"
            );
        }
    }
}
