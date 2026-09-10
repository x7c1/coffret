/// The caller-owned values one call must not write down.
///
/// A gateway knows what it was configured with — a bucket, a base prefix, a
/// folder somebody picked — and a provider is free to echo any of it back in
/// the text of a refusal. There is rarely only one such value: an S3 call
/// carries both the bucket and the prefix, so a rule built around a single
/// value leaves whichever one it was not given in the log (spec: EL-5).
///
/// Built rather than parsed, so that a site which records provider text has to
/// name what is private about the call it is recording — including by naming
/// nothing, with [`PrivateValues::none`], which is the declaration that the
/// call addressed one opaque object.
///
/// ```
/// use coffret_logging::redact::{text_without, PrivateValues};
///
/// let private = PrivateValues::none()
///     .with("someones-photos")
///     .with("people/alice/Summer Library");
/// let safe = text_without(
///     "PUT /someones-photos/people/alice/Summer Library/head-1.cfrt failed",
///     &private,
/// );
///
/// assert_eq!(safe, "PUT /[redacted]/[redacted]/head-1.cfrt failed");
/// ```
#[derive(Debug, Clone, Default)]
pub struct PrivateValues {
    /// Longest first, so that a value which contains another is taken out
    /// whole before the shorter one could cut it into a fragment.
    values: Vec<String>,
}

impl PrivateValues {
    /// A call with no private value of its own.
    ///
    /// Applying this removes nothing, which is the point: the site still says
    /// what it is doing, and a later call that *does* address a configured
    /// location cannot be added without answering the same question.
    pub fn none() -> Self {
        Self::default()
    }

    /// Adds one value to keep out of whatever text is recorded.
    ///
    /// An empty value adds nothing — a gateway configured at the root of a
    /// bucket has no prefix, and an empty needle would match everywhere.
    pub fn with(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        if value.is_empty() {
            return self;
        }
        self.values.push(value);
        // Insertion order is the caller's convenience; length order is what
        // decides the result, so it is settled here rather than at each use.
        self.values.sort_by(|left, right| {
            right
                .len()
                .cmp(&left.len())
                .then_with(|| left.as_str().cmp(right.as_str()))
        });
        self
    }

    /// The values to remove, longest first.
    pub(super) fn longest_first(&self) -> &[String] {
        &self.values
    }
}

#[cfg(test)]
mod tests {
    use super::super::{body_without, text_without, REDACTED};
    use super::*;

    #[test]
    fn a_bucket_and_a_prefix_are_both_taken_out_of_one_body() {
        let private = PrivateValues::none()
            .with("someones-holiday-photos")
            .with("people/alice/Summer Library");
        let refusal = br#"<Error><Code>AccessDenied</Code><Message>Access to someones-holiday-photos denied for people/alice/Summer Library/head-1.cfrt</Message></Error>"#;

        let safe = body_without(refusal, &private);

        assert!(!safe.contains("someones-holiday-photos"), "{safe}");
        assert!(!safe.contains("alice"), "{safe}");
        // What the provider said is still what the event carries.
        assert!(safe.contains("AccessDenied"), "{safe}");
        assert!(safe.contains("head-1.cfrt"), "{safe}");
    }

    // A prefix that begins with the bucket's own name is the ordinary shape of
    // an S3 arrangement written out in full. Taken shortest-first, the bucket
    // would be replaced inside the prefix and the rest of the prefix would
    // then match nothing — leaving `[redacted]/Summer Library` in the log.
    #[test]
    fn the_longest_private_value_goes_first_so_no_fragment_is_left() {
        let private = PrivateValues::none()
            .with("holiday-photos")
            .with("holiday-photos/Summer Library");

        let safe = text_without("no listing at holiday-photos/Summer Library", &private);

        assert_eq!(safe, format!("no listing at {REDACTED}"));
    }

    // The empty set is what a call addressing one opaque object declares, and
    // declaring it must not change the evidence.
    #[test]
    fn no_private_value_is_the_identity() {
        let refusal = "<Error><Code>NoSuchBucket</Code></Error>";

        assert_eq!(text_without(refusal, &PrivateValues::none()), refusal);
        // Adding nothing is still nothing: a Library at the root of a bucket
        // has an empty prefix, and it arrives here as one.
        let empty_prefix = PrivateValues::none().with("");
        assert_eq!(text_without(refusal, &empty_prefix), refusal);
        assert!(empty_prefix.longest_first().is_empty());
    }
}
