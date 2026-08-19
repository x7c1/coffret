/// Where in Drive one Library lives, and how it is read.
///
/// Credentials are not here: they reach the gateway as an
/// [`AccessTokens`](crate::AccessTokens) implementation, which is what keeps
/// the store itself free of any opinion about how a grant was obtained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveSettings {
    folder_id: String,
    page_size: i32,
}

/// How many files a listing asks for at a time when nothing else is said.
///
/// Drive's own ceiling, so the default costs one call per thousand objects.
const DEFAULT_PAGE_SIZE: i32 = 1000;

impl DriveSettings {
    /// Puts a Library in one Drive folder.
    ///
    /// The folder is one this application created, because the grant coffret
    /// asks for reaches nothing else.
    pub fn new(folder_id: impl Into<String>) -> Self {
        Self {
            folder_id: folder_id.into(),
            page_size: DEFAULT_PAGE_SIZE,
        }
    }

    /// Sets how many objects one listing page holds.
    ///
    /// # Panics
    ///
    /// If `page_size` is not positive: Drive rejects such a request, and a
    /// listing whose pages hold nothing would never reach its end.
    pub fn with_page_size(mut self, page_size: i32) -> Self {
        assert!(
            page_size > 0,
            "a listing page must hold at least one object"
        );
        self.page_size = page_size;
        self
    }

    /// The folder the Library's objects are in.
    pub fn folder_id(&self) -> &str {
        &self.folder_id
    }

    /// How many objects one listing page holds.
    pub fn page_size(&self) -> i32 {
        self.page_size
    }
}
