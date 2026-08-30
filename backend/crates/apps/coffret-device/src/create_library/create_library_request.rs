/// Everything a new Library is created from.
///
/// The Passphrase is not among them. It reaches
/// [`create_library`](super::create_library) through a callback instead, so that
/// every refusal this request can earn — a name that is not one path component,
/// a Library of that name already here, a base prefix that runs into the
/// Library's own folder name — is made before anybody is asked to choose one.
#[derive(Debug)]
pub struct CreateLibraryRequest {
    /// What this device is to call the Library.
    ///
    /// A device-side name and not a Library-wide one: it is this directory's
    /// name, and another device holding the same Library may call it something
    /// else.
    pub name: String,
    /// Where the Library is to live.
    pub provider: NewProvider,
}

/// Where a Library about to be created is to live.
///
/// Apart from [`ProviderSettings`](crate::ProviderSettings) because the two say
/// different things: this is what a person asked for — a parent folder, a base
/// prefix — and that is what the Library turned out to be, which is only known
/// once the app folder exists and the Library ID has been drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewProvider {
    /// A folder on Google Drive.
    Drive {
        /// The folder the Library's app folder goes in.
        ///
        /// Required, and deliberately not optional: a folder created at the top
        /// of My Drive is never where a person wanted an application to put one,
        /// and it is the placement Drive gives anything created without a parent.
        parent: String,
        /// The OAuth client to authorize as. It has to be a desktop client:
        /// the flow redirects to a loopback port the operating system picks,
        /// which a web client cannot be registered for.
        client_id: String,
        /// The client secret, for a client registered with one.
        client_secret: Option<String>,
    },
    /// A prefix of an S3 bucket.
    S3 {
        /// The bucket to keep the Library in.
        bucket: String,
        /// Where in the bucket to put it: the empty string for the bucket root,
        /// otherwise a prefix ending in `/`. The Library's own prefix is this
        /// with `coffret-<library id>/` after it (spec: FM-18).
        base_prefix: String,
        /// The S3 endpoint to talk to, where it is not AWS's own.
        endpoint: Option<String>,
        /// The region to sign for, where the SDK's own resolution is not to
        /// decide it.
        region: Option<String>,
        /// Whether the bucket is addressed as a path segment rather than as a
        /// subdomain.
        path_style: bool,
    },
}
