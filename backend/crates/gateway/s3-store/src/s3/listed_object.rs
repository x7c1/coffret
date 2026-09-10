use coffret_usecase::{Error, ObjectInfo, ObjectRef, ProviderHash, Result};

use crate::key_layout::KeyLayout;

/// Turns one entry of a listing into what the port reports, where it is one of
/// this Library's objects at all.
///
/// A key outside this Library's layout is `None` and skipped: asking S3 to
/// collapse keys past a separator already keeps the trash out, and this keeps a
/// stray key someone else wrote under the prefix from being reported as a
/// Storage Object.
///
/// A listed object with no key at all is a different thing and is refused. The
/// key is what the listing is a listing of, so an entry without one is not a
/// key this Library has no reading for — it is S3 answering with something
/// other than a listing, and skipping it would report a Library one object
/// short of what Storage holds.
///
/// # Errors
///
/// [`Error::MalformedResponse`] where the listed object carries no key.
pub(crate) fn describe(
    layout: &KeyLayout,
    object: &aws_sdk_s3::types::Object,
) -> Result<Option<ObjectInfo>> {
    let Some(key) = object.key() else {
        // An entry with no key has nothing worth naming, and the prefix it came
        // out of is the person's own arrangement of their Storage rather than
        // evidence of anything (spec: EL-1, EL-5). What the entry has to say is
        // that it arrived without the one field a listing is a listing of.
        return Err(Error::MalformedResponse {
            detail: "Storage listed an object with no key".to_owned(),
        });
    };
    let Some(name) = layout.name_of(key) else {
        return Ok(None);
    };
    Ok(Some(ObjectInfo {
        object_ref: ObjectRef::new(name),
        name: name.to_owned(),
        // S3 quotes its ETags; the quotes are transport syntax, not part of the
        // digest, and leaving them in would make the value fail to compare
        // against anything computed locally.
        hash: object
            .e_tag()
            .map(|tag| ProviderHash::new(tag.trim_matches('"'))),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The prefix the cases' layout is configured with.
    const PREFIX: &str = "libraries/alpha";

    /// The layout the cases below read a listing against.
    fn layout() -> KeyLayout {
        KeyLayout::new(PREFIX)
    }

    // A key someone else wrote under this Library's prefix, and the trash this
    // gateway makes out of the key space: neither is a Storage Object, and a
    // listing that reported them would hand the commit flow names it never
    // wrote.
    #[test]
    fn a_key_outside_the_layout_is_left_out_of_the_listing() {
        for key in [
            "libraries/alpha/trash/head-1.cfrt",
            "libraries/beta/head-1.cfrt",
        ] {
            let listed = aws_sdk_s3::types::Object::builder().key(key).build();

            let described = describe(&layout(), &listed);
            assert!(
                matches!(described, Ok(None)),
                "expected {key} to be skipped, got {described:?}",
            );
        }
    }

    #[test]
    fn a_listed_object_without_a_key_is_a_malformed_response() {
        let listed = aws_sdk_s3::types::Object::builder().build();

        let described = describe(&layout(), &listed);
        let Err(Error::MalformedResponse { detail }) = &described else {
            panic!("expected an entry with no key to be refused, got {described:?}");
        };
        // What the entry is refused for is what it is missing, never where it
        // was listed from: the prefix is the person's own arrangement of their
        // Storage rather than evidence, and this refusal is rendered into the
        // log as it stands (spec: EL-1, EL-5).
        assert!(
            !detail.contains(PREFIX),
            "the configured prefix must stay out of the refusal: {detail}"
        );
    }
}
