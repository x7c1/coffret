use std::fmt;

/// Which step of creating or joining a Library a failure happened at.
///
/// What went wrong is in the cause; this says what was being attempted, which
/// is what tells a Passphrase that could not be stored apart from a grant that
/// was never given and from a catalog that could not be made.
///
/// The two flows share it because they are the same sequence over a Library
/// that does not exist yet and one that does: only the app-folder step differs,
/// and it differs in direction — one flow creates the folder, the other reads
/// back the name of one that is already there.
///
/// Every step here is one a staging directory is open for, which is why drawing
/// the Library ID and asking the bucket whether it is there are not among them:
/// both are settled before anything is staged, and each answers with a failure
/// of its own — [`Error::KeyMaterial`] and [`Error::BucketUnreachable`] — rather
/// than with a Library that was not created.
///
/// [`Error::KeyMaterial`]: super::Error::KeyMaterial
/// [`Error::BucketUnreachable`]: super::Error::BucketUnreachable
#[derive(Debug)]
pub enum CreationStep {
    /// Writing the Master Key under the Passphrase.
    StoredMasterKey,
    /// Asking the person for a grant on the Storage provider, or reaching the
    /// grant of the account the device already holds.
    Authorization,
    /// Writing the envelope that opens the account's grant for this Library
    /// (spec: SA-9).
    AccountEnvelope,
    /// Creating the Library's app folder (spec: FM-18).
    AppFolder,
    /// Reading the name of the app folder a Library was said to live in
    /// (spec: FM-18).
    AppFolderName,
    /// Asking the place a Library was said to live in whether it holds what a
    /// Library keeps at the top of its own place (spec: FM-12).
    ///
    /// Only where that question needs a grant, which is Drive: on S3 it is
    /// asked before anything is staged and answers with
    /// [`Error::BucketUnreachable`], as the bucket check beside it does.
    ///
    /// [`Error::BucketUnreachable`]: super::Error::BucketUnreachable
    LibraryObject,
    /// Creating the catalog.
    Index,
    /// Creating the spool the encrypted files wait to be uploaded from.
    Spool,
    /// Writing the settings file.
    Settings,
    /// Moving the finished directory into place.
    Publish,
}

impl fmt::Display for CreationStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let said = match self {
            Self::StoredMasterKey => "storing the Master Key under the Passphrase",
            Self::Authorization => "asking for a grant on the Storage provider",
            Self::AccountEnvelope => "writing the envelope that opens the account's grant",
            Self::AppFolder => "creating the Library's app folder",
            Self::AppFolderName => "reading the name of the Library's app folder",
            Self::LibraryObject => "asking whether the Library's app folder holds anything of it",
            Self::Index => "creating the catalog",
            Self::Spool => "creating the spool directory",
            Self::Settings => "writing the settings file",
            Self::Publish => "moving the finished Library directory into place",
        };
        f.write_str(said)
    }
}
