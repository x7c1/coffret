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
        // An entry with no key has nothing else worth naming, so what the
        // refusal names is the listing it came out of.
        return Err(Error::MalformedResponse {
            detail: format!(
                "Storage listed an object with no key under {:?}",
                layout.live_prefix()
            ),
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

    /// The layout the cases below read a listing against.
    fn layout() -> KeyLayout {
        KeyLayout::new("libraries/alpha")
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

        assert!(matches!(
            describe(&layout(), &listed),
            Err(Error::MalformedResponse { .. })
        ));
    }
}
