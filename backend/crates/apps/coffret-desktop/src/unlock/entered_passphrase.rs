use std::fmt;

use coffret_device::Passphrase;
use serde::de::{self, Deserialize, Deserializer, Visitor};
use zeroize::ZeroizeOnDrop;

/// The Passphrase as the Passphrase window's call hands it to this shell: the
/// one point where coffret's own code receives it, and so where DK-7's claim
/// over it begins.
///
/// It is read from the call's arguments straight into a [`Passphrase`], with
/// no `String` in between. Tauri deserializes a command's arguments from the
/// message the page sent, which it holds as a JSON value and lends out; the
/// bytes are copied from that value into the Passphrase's own buffer and
/// nowhere else, so the first copy coffret holds is already the one that is
/// wiped. What came before — the field's value and the script's string in the
/// webview, the message on its way through Tauri's IPC — is outside anything
/// coffret can overwrite, and outside the claim (spec: DK-7).
///
/// It is on the secret-bearing inventory in `coffret_model::MasterKey`'s
/// module. It carries no `Drop` of its own: the Passphrase in it wipes itself,
/// and a `Drop` here would forbid the one thing a command does with it — moving
/// that Passphrase out. It is not `Clone`, and it has no `Debug` or `Display`.
pub struct EnteredPassphrase(Passphrase);

impl EnteredPassphrase {
    /// The Passphrase, moved out for the unlock that spends it.
    pub fn into_passphrase(self) -> Passphrase {
        self.0
    }
}

impl ZeroizeOnDrop for EnteredPassphrase {}

impl<'de> Deserialize<'de> for EnteredPassphrase {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_string(PassphraseVisitor)
    }
}

/// Reads a string argument into a [`Passphrase`].
struct PassphraseVisitor;

impl Visitor<'_> for PassphraseVisitor {
    type Value = EnteredPassphrase;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the Passphrase, as a string")
    }

    // What Tauri's arguments offer: a string borrowed from the message, copied
    // here into the buffer that is wiped and into nothing else.
    fn visit_str<E: de::Error>(self, typed: &str) -> Result<Self::Value, E> {
        Ok(EnteredPassphrase(Passphrase::from_bytes(
            typed.as_bytes().to_vec(),
        )))
    }

    // A deserializer that owns its string hands it over whole, and its
    // allocation becomes the Passphrase's.
    fn visit_string<E: de::Error>(self, typed: String) -> Result<Self::Value, E> {
        Ok(EnteredPassphrase(Passphrase::from_bytes(
            typed.into_bytes(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::marker::PhantomData;

    use serde::de::value::{Error, StringDeserializer};
    use serde::de::IntoDeserializer;

    use super::*;

    #[test]
    fn a_string_argument_becomes_the_passphrase_typed() {
        // Borrowed from a JSON value, the way Tauri lends a command's argument.
        let message = serde_json::json!({ "passphrase": "correct horse" });
        let entered = EnteredPassphrase::deserialize(&message["passphrase"]).unwrap();
        assert_eq!(entered.into_passphrase().as_bytes(), b"correct horse");

        // Handed over whole.
        let owned: StringDeserializer<Error> = "battery staple".to_owned().into_deserializer();
        let entered = EnteredPassphrase::deserialize(owned).unwrap();
        assert_eq!(entered.into_passphrase().as_bytes(), b"battery staple");
    }

    #[test]
    fn anything_but_a_string_is_refused() {
        let message = serde_json::json!({ "passphrase": 7 });
        assert!(EnteredPassphrase::deserialize(&message["passphrase"]).is_err());
    }

    /// Accepts only a type that wipes its bytes when it is dropped.
    const fn zeroizes_on_drop<T: ZeroizeOnDrop>() {}

    /// Answers whether `T` is `Clone`, the way the inventory's assertions do.
    struct Probe<T>(PhantomData<T>);

    trait NotClone {
        fn is_clone() -> bool {
            false
        }
    }

    impl<T> NotClone for Probe<T> {}

    impl<T: Clone> Probe<T> {
        fn is_clone() -> bool {
            true
        }
    }

    // The testable half of DK-7 for the one inventory type the domain's
    // assertions cannot name: they live in a crate below this shell. The same
    // two checks, for the same reasons; the probe is checked against a type that
    // is `Clone` on purpose, so that it can tell the two apart.
    #[test]
    fn the_entered_passphrase_is_secret_bearing() {
        zeroizes_on_drop::<EnteredPassphrase>();
        assert!(!Probe::<EnteredPassphrase>::is_clone());
        assert!(Probe::<String>::is_clone());
    }
}
