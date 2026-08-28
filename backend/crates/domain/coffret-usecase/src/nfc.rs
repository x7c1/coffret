use std::borrow::Cow;

use unicode_normalization::{is_nfc, UnicodeNormalization};

/// Text from outside the Library, in the form every Entry Path is spelled in
/// (spec: EP-1).
///
/// The rule this stands for: **text from outside the Library normalizes on the
/// way in; text the Library already holds is already NFC.** A name read off a
/// disk and the prefix a device's mapping is configured with are both from
/// outside, and the two spellings are not a matter of taste — one filesystem
/// hands `é` back as a single code point and another as `e` followed by a
/// combining acute, for the very same file. The catalog compares the bytes it is
/// given and folds nothing together (spec: EP-3), so without this the same file
/// would take a different Entry Path on each of the two devices, and a fetch
/// that wrote the composed name would have its own file reported as a new one by
/// the next scan.
///
/// Nothing decoded from a stored object passes through here, and that is the
/// other half of the rule. EP-1 already holds of what a Journal record, an Index
/// Snapshot, or a Container's metadata carries, so normalizing on the way back
/// in would rewrite bytes that were hashed and signed as they stand — a stored
/// path that is not NFC is a question for the format layer, not something to
/// paper over here.
pub(crate) fn nfc(text: &str) -> Cow<'_, str> {
    if is_nfc(text) {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(text.nfc().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_decomposed_name_is_composed() {
        assert_eq!(nfc("cafe\u{301}.jpg"), "caf\u{e9}.jpg");
    }

    // The ordinary case, and the reason this answers with a `Cow`: almost every
    // name a scan reads is already NFC, and none of them is worth a copy.
    #[test]
    fn a_name_already_in_nfc_is_handed_back_as_it_is() {
        assert!(matches!(
            nfc("caf\u{e9}.jpg"),
            Cow::Borrowed("caf\u{e9}.jpg")
        ));
        assert!(matches!(nfc("a.jpg"), Cow::Borrowed("a.jpg")));
    }
}
