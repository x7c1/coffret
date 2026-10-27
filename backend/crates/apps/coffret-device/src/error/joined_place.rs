use std::fmt;

/// Where a join was given to look for a Library: what a failed join says it
/// was joining, since a join names no Library (spec: FM-15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinedPlace {
    /// A folder on Google Drive, by the id Drive minted for it.
    DriveFolder {
        /// The folder's id, as it was given.
        folder_id: String,
    },
    /// A prefix of an S3 bucket.
    S3 {
        /// The bucket.
        bucket: String,
        /// The prefix, as it was given.
        prefix: String,
    },
}

impl fmt::Display for JoinedPlace {
    /// The place by what it is: the folder, or the prefix and its bucket.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DriveFolder { folder_id } => write!(f, "the Google Drive folder {folder_id:?}"),
            Self::S3 { bucket, prefix } => {
                write!(f, "the prefix {prefix:?} of the S3 bucket {bucket:?}")
            }
        }
    }
}
