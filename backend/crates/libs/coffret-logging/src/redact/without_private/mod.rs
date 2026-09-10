use super::private_values::PrivateValues;

mod percent_encode;
use percent_encode::percent_encode;

mod without_token;
use without_token::without_token;

/// Replaces the caller-owned private values wherever any of them stands as a
/// whole token.
///
/// A provider echoes what it was asked for, and the value as the caller knows
/// it is not the only spelling that can come back: an HTTP client may have
/// percent-encoded it on the way out, so every representation the request could
/// have carried is taken out too. The values are taken longest first, so a
/// prefix that begins with the bucket's own name is removed whole rather than
/// cut into a fragment by the shorter value inside it.
///
/// Which spellings those are lives in [`percent_encode`], and where an
/// occurrence may be replaced at all in [`without_token`], so that adding a
/// spelling and revising the boundary stay separate changes.
pub(super) fn without_private(text: &str, private: &PrivateValues) -> String {
    let mut safe = text.to_owned();
    for value in private.longest_first() {
        safe = without_one(&safe, value);
    }
    safe
}

/// Takes one value, and every representation a request could have carried it
/// in, out of the text.
fn without_one(text: &str, private: &str) -> String {
    let mut safe = without_token(text, private);
    for preserve_slash in [false, true] {
        for space_as_plus in [false, true] {
            for uppercase_hex in [false, true] {
                let encoded = percent_encode(private, preserve_slash, space_as_plus, uppercase_hex);
                if encoded != private {
                    safe = without_token(&safe, &encoded);
                }
            }
        }
    }

    safe
}

#[cfg(test)]
mod tests {
    use super::super::REDACTED;
    use super::*;

    /// The one value a case keeps out.
    fn private(value: &str) -> PrivateValues {
        PrivateValues::none().with(value)
    }

    #[test]
    fn a_uri_encoded_private_value_is_removed() {
        let prefix = private("people/alice/Summer Library");

        for text in [
            "URI=/people/alice/Summer%20Library/entry",
            "query=people%2falice%2fSummer+Library",
        ] {
            let safe = without_private(text, &prefix);

            assert!(!safe.contains("alice"), "{safe}");
            assert!(safe.contains(REDACTED), "{safe}");
        }
    }

    // S3 allows a three-character bucket name, so the shortest configuration
    // anybody can have is exactly the one a length threshold would abandon.
    #[test]
    fn a_three_character_bucket_is_taken_out_where_it_stands_as_a_token() {
        let bucket = private("log");

        for text in [
            "log",
            "no bucket named log",
            "PUT /log/head-1.cfrt failed",
            "bucket=log&region=us-east-1",
        ] {
            let safe = without_private(text, &bucket);

            assert!(safe.contains(REDACTED), "{safe}");
        }
    }

    // The other half of the same rule: replacing a short value everywhere it
    // appeared would turn the provider's own words into wreckage, and those
    // words are the evidence the event exists for.
    #[test]
    fn a_three_character_bucket_inside_a_providers_own_word_is_left_alone() {
        let bucket = private("log");
        let refusal = "check the logging configuration; the catalog is unreadable";

        let safe = without_private(refusal, &bucket);

        assert_eq!(safe, refusal);
    }

    // A value named twice, once inside a longer word and once on its own: the
    // occurrence that is left alone must not stop the walk before the one that
    // is not.
    #[test]
    fn a_later_occurrence_is_still_reached_past_one_that_is_left_alone() {
        let safe = without_private("logging uses log", &private("log"));

        assert_eq!(safe, format!("logging uses {REDACTED}"));
    }

    #[test]
    fn an_empty_set_removes_nothing() {
        assert_eq!(
            without_private("Storage answered", &PrivateValues::none()),
            "Storage answered"
        );
    }
}
