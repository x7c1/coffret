use std::fmt;

/// What a Storage operation asked for and did not find.
///
/// [`Error::NotFound`](crate::Error::NotFound) carries one of these rather than
/// a name, because not every call that comes back empty-handed is about an
/// object. A listing addresses the place a Library was configured into, a
/// pre-store check addresses the bucket or the folder itself, and the endpoint
/// that mints identifiers addresses nothing of the Library's at all.
///
/// Naming the kind keeps the wording in one place: [`Display`] is what a
/// person reads, and [`Self::subject`] is what a diagnostic event's `object`
/// field is built from, so that field stays queryable and stays free of the
/// bucket, the prefix, or the folder somebody chose (spec: EL-5).
///
/// [`Display`]: fmt::Display
#[derive(Debug, Clone)]
pub enum Missing {
    /// One object, by the name coffret minted or the provider minted — a
    /// Container's opaque one and a control object's recognizable one alike.
    Object(String),
    /// The Library's listing, which addresses the configured location rather
    /// than any one object.
    Listing,
    /// The bucket, or the provider folder, the Library was configured into.
    Location,
    /// A provider resource that names no object of the Library's — the
    /// identifier-minting endpoint, for one.
    Endpoint,
}

impl Missing {
    /// What a diagnostic event records as the subject of the failure.
    ///
    /// An object is named: its name is one coffret or the provider minted, and
    /// such a name is evidence an event may keep (spec: EL-5). Every other
    /// kind answers with a fixed word, because what would otherwise fill the
    /// field is somebody's own arrangement of their Storage rather than
    /// anything either side minted.
    pub fn subject(&self) -> &str {
        match self {
            Self::Object(name) => name,
            Self::Listing => "the listing",
            Self::Location => "the configured location",
            Self::Endpoint => "the endpoint",
        }
    }
}

impl fmt::Display for Missing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Object(name) => write!(f, "no object named {name:?} in Storage"),
            Self::Listing => write!(f, "Storage holds no listing for this Library"),
            Self::Location => write!(f, "Storage holds no bucket or folder for this Library"),
            Self::Endpoint => write!(f, "Storage answered a call with nowhere for it to go"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A listing addresses the prefix or the folder a Library was configured
    // into, so there is no name to report and the one that would have been
    // reported is the person's own. What is said instead is what was asked
    // for.
    #[test]
    fn a_listing_that_is_not_there_is_reported_without_a_name() {
        let missing = Missing::Listing;

        assert_eq!(missing.subject(), "the listing");
        assert_eq!(
            missing.to_string(),
            "Storage holds no listing for this Library"
        );
    }

    // The bucket somebody typed and the folder they picked out of their own
    // Drive are the same case: the pre-store check knows which one it asked
    // about, and the person who configured it does too.
    #[test]
    fn a_configured_location_that_is_not_there_is_reported_without_being_named() {
        let missing = Missing::Location;

        assert_eq!(missing.subject(), "the configured location");
        assert_eq!(
            missing.to_string(),
            "Storage holds no bucket or folder for this Library"
        );
    }

    // The one subject that is named, and the rendering that has to stay as it
    // was: a Container's name is opaque (spec: FM-3), so it is both what a
    // report says and what an event records.
    #[test]
    fn an_object_that_is_not_there_is_still_reported_by_its_opaque_name() {
        let name = "0123456789abcdef0123456789abcdef.cfrt";
        let missing = Missing::Object(name.to_owned());

        assert_eq!(missing.subject(), name);
        assert_eq!(
            missing.to_string(),
            "no object named \"0123456789abcdef0123456789abcdef.cfrt\" in Storage"
        );
    }

    // A 404 from the call that mints an identifier is about the provider's own
    // endpoint: nothing of this Library's was named, so nothing of it is
    // reported.
    #[test]
    fn a_call_that_names_no_object_is_reported_as_the_endpoint() {
        let missing = Missing::Endpoint;

        assert_eq!(missing.subject(), "the endpoint");
        assert_eq!(
            missing.to_string(),
            "Storage answered a call with nowhere for it to go"
        );
    }
}
