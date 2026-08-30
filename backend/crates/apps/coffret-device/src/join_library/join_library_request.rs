/// Everything a device joining an existing Library is told.
///
/// No `Debug`, and that is the point of the type: the Recovery Code is the
/// Master Key in the form a person writes down (spec: KD-11), so a request
/// holding one must not be printable by accident. The Passphrase is not here at
/// all — it reaches [`join_library`](super::join_library) through a callback, so
/// that every refusal needing no key is made before a person is asked for one.
pub struct JoinLibraryRequest {
    /// What this device is to call the Library.
    ///
    /// A device-side name and not a Library-wide one: the device that created
    /// the Library may call it something else, the way it may map its folders
    /// differently (spec: CK-7).
    pub name: String,
    /// The Recovery Code, as it was typed.
    ///
    /// Grouped, ungrouped, upper case or lower: the format crate reads all of
    /// them and refuses anything that is not one of them, so what a person wrote
    /// down is what they may type back (spec: KD-11).
    pub recovery_code: String,
    /// Where the Library already lives.
    pub provider: JoinedProvider,
}

/// Where a Library this device is joining already is.
///
/// Both variants name the app folder itself rather than what it is under, which
/// is the difference from creating one: the folder exists, another device made
/// it, and the Library ID is read back out of its name (spec: FM-18). Finding it
/// from the Recovery Code alone — enumerating the `coffret-` folders at a
/// Storage location — is the restore flow's, and until that exists a person
/// pastes what `init` printed or what the provider's own interface shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinedProvider {
    /// The Library's folder on Google Drive.
    Drive {
        /// The app folder itself, by the id Drive minted for it.
        ///
        /// Drive names files by id and not by name, so this is the whole of what
        /// says which folder; its *name* is what says which Library, and the
        /// join reads that back before it records anything.
        folder_id: String,
        /// The OAuth client to authorize as. It has to be a desktop client:
        /// the flow redirects to a loopback port the operating system picks,
        /// which a web client cannot be registered for.
        client_id: String,
        /// The client secret, for a client registered with one.
        client_secret: Option<String>,
    },
    /// The Library's prefix of an S3 bucket.
    S3 {
        /// The bucket the Library is in.
        bucket: String,
        /// The Library's own prefix, ending in `coffret-<library id>/`.
        ///
        /// The whole prefix and not the base under it: which Library this is has
        /// to be settled by what was typed rather than guessed at, and a base
        /// alone would name every Library kept at that location (spec: FM-18).
        prefix: String,
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
