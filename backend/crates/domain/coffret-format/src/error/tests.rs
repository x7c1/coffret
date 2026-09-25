//! What each [`Error`] variant says, what it carries under that, and what a
//! diagnostic event is told of it.

use std::error;

use coffret_model::Redacted;

use super::*;

/// The links a caller printing `{error:#}` reads, outermost first.
fn chain(error: &dyn error::Error) -> Vec<String> {
    let mut links = vec![error.to_string()];
    let mut below = error.source();
    while let Some(link) = below {
        links.push(link.to_string());
        below = link.source();
    }
    links
}

// A length this build cannot address is said as that and nothing more: the
// sentence accuses no bytes, because a 64-bit build opens what a 32-bit one
// refuses. And a chunk size of zero, which is a defect of the object, no
// longer shares a sentence with it.
#[test]
fn a_length_this_build_cannot_address_accuses_no_bytes() {
    let refused = Error::UnaddressableOnThisBuild {
        what: "chunk size",
        declared: 1 << 40,
    };
    assert_eq!(
        refused.to_string(),
        "a chunk size of 1099511627776 bytes is past what this build can address",
    );
    assert_eq!(Error::InvalidChunkSize.to_string(), "chunk size is zero");
}

// What the format layer says about bytes is worth having in a log, and
// saying it costs nothing: an object names nothing a person chose.
#[test]
fn what_the_format_layer_says_about_bytes_is_kept() {
    assert_eq!(
        Error::AuthenticationFailed.redacted(),
        "Format: message failed authentication",
    );
}

// The one way a path could reach a diagnostic event through this
// vocabulary.
#[test]
fn a_domain_refusal_underneath_is_redacted_rather_than_quoted() {
    let error = Error::Model(coffret_model::Error::UnnormalizedEntryPath {
        path: "albums/spring.jpg".to_owned(),
    });

    // The path is the domain's own answer and reaches a person under this
    // line rather than inside it, which is what keeps the chain from
    // saying one refusal twice.
    assert_eq!(
        chain(&error),
        vec![
            "a value in a meta section or a control object is not one the domain admits".to_owned(),
            "the stored path \"albums/spring.jpg\" is not normalized to NFC".to_owned(),
        ],
    );
    assert_eq!(
        error.redacted(),
        "Format::Model: Model::UnnormalizedEntryPath(path_len=17)",
    );
}

// The entropy source's own refusal: the value travels, so the chain
// reaches what it said rather than stopping at a sentence this crate
// rendered.
#[test]
fn an_entropy_failure_carries_what_the_source_reported() {
    let reported = getrandom::Error::UNSUPPORTED;
    let error = Error::EntropyUnavailable { cause: reported };

    // The value itself under the chain and not merely something under it:
    // what the field is for is a caller reading the kind the source named,
    // which a rendered sentence could not have answered.
    let source = error::Error::source(&error).expect("the chain reaches the source");
    assert!(
        source.downcast_ref::<getrandom::Error>().is_some(),
        "the source is the value getrandom reported and not a rendering of it",
    );
    // Said once: this line names the step, and what the source reported is
    // the link under it.
    assert_eq!(
        chain(&error),
        vec![
            "could not draw random bytes".to_owned(),
            reported.to_string()
        ],
    );
    // Composed from the source's own rendering rather than written out: a
    // reworded upstream sentence is not this layer's rendering changing. A
    // diagnostic event has no chain to walk, so this is the one rendering
    // that still spells the cause out.
    assert_eq!(
        error.redacted(),
        format!("Format: could not draw random bytes: {reported}"),
    );
}

// The same for the Argon2id implementation's refusal: which parameter it
// would not take is an enum variant, and a caller can read it off the chain
// rather than off a sentence.
#[test]
fn an_argon2id_refusal_carries_what_the_implementation_reported() {
    let reported = argon2::Error::MemoryTooLittle;
    let error = Error::InvalidArgon2Params { cause: reported };

    let source = error::Error::source(&error).expect("the chain reaches the source");
    assert!(
        source.downcast_ref::<argon2::Error>().is_some(),
        "the source is the value Argon2id reported and not a rendering of it",
    );
    assert_eq!(
        chain(&error),
        vec![
            "invalid Argon2id parameters".to_owned(),
            reported.to_string()
        ],
    );
    // Composed from the source's own rendering, as above.
    assert_eq!(
        error.redacted(),
        format!("Format: invalid Argon2id parameters: {reported}"),
    );

    // The derivation's own refusal is the same arrangement: the two share
    // the `source()` arm but render apart, so both of the second one's
    // renderings are read back here rather than taken on the first one's
    // word. A salt is what that call refuses over — a memory cost is
    // `Params::new`'s to refuse, and so the other variant's.
    let refused = argon2::Error::SaltTooShort;
    let derivation = Error::PassphraseDerivationFailed { cause: refused };
    let reached = error::Error::source(&derivation).expect("the chain reaches the source");
    assert!(reached.downcast_ref::<argon2::Error>().is_some());
    assert_eq!(
        chain(&derivation),
        vec![
            "could not derive the protection key".to_owned(),
            refused.to_string(),
        ],
    );
    assert_eq!(
        derivation.redacted(),
        format!("Format: could not derive the protection key: {refused}"),
    );
}
