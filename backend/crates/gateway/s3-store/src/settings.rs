use std::time::Duration;

use crate::call_deadline::SMALL_CALL_DEADLINE;

/// Where in S3 one Library lives, and how it is read.
///
/// Credentials, region, and endpoint are not here: they belong to the
/// `aws_sdk_s3::Client` the caller builds and hands to
/// [`S3::new`](crate::S3::new), which is also what lets a test point the same
/// gateway at MinIO without the gateway knowing MinIO exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3Settings {
    bucket: String,
    prefix: String,
    page_size: i32,
    small_call_deadline: Duration,
}

/// How many objects a listing asks for at a time when nothing else is said.
///
/// The same number S3 itself caps `ListObjectsV2` at, so the default costs one
/// request per thousand objects rather than silently paging more often.
const DEFAULT_PAGE_SIZE: i32 = 1000;

impl S3Settings {
    /// Puts a Library at the root of `bucket`.
    pub fn new(bucket: impl Into<String>) -> Self {
        Self {
            bucket: bucket.into(),
            prefix: String::new(),
            page_size: DEFAULT_PAGE_SIZE,
            small_call_deadline: SMALL_CALL_DEADLINE,
        }
    }

    /// Puts the Library under a prefix of the bucket instead of at its root.
    ///
    /// A Library's own prefix is its app folder's: the base the user configured
    /// with `coffret-<library id>/` appended, which is what
    /// `LibraryId::app_prefix` builds (spec: FM-18). Nothing here creates it —
    /// on S3 a prefix exists by being written under.
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Sets how many objects one listing page holds.
    ///
    /// Worth lowering only to make paging happen where it otherwise would not —
    /// which is what the conformance suite does to reach its second page
    /// without writing a thousand objects first.
    ///
    /// # Panics
    ///
    /// If `page_size` is not positive: S3 rejects such a request, and a listing
    /// whose pages hold nothing would never reach its end.
    pub fn with_page_size(mut self, page_size: i32) -> Self {
        assert!(
            page_size > 0,
            "a listing page must hold at least one object"
        );
        self.page_size = page_size;
        self
    }

    /// Holds the calls to another deadline than [`SMALL_CALL_DEADLINE`].
    ///
    /// Worth changing only to make the deadline fire where waiting out the real
    /// one would be pointless — which is what the cases about it do. A call
    /// sending a body is given this plus the time its body takes; the body of a
    /// `get`'s answer and the copy a trash makes are unaffected. See
    /// [`call_deadline`](crate::call_deadline) for which call is held how.
    pub fn with_small_call_deadline(mut self, deadline: Duration) -> Self {
        self.small_call_deadline = deadline;
        self
    }

    /// The bucket the Library is stored in.
    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    /// The prefix every key of the Library starts with.
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    /// How many objects one listing page holds.
    pub fn page_size(&self) -> i32 {
        self.page_size
    }

    /// How long a call whose body is small may take altogether, and the base
    /// of the deadline for one that sends a body.
    pub fn small_call_deadline(&self) -> Duration {
        self.small_call_deadline
    }
}
