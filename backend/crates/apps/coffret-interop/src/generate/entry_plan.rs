use coffret_model::{ContainerId, DerivedFrom, EntryPath};

/// One Entry to write, with the content it owns.
pub(super) struct EntryPlan {
    pub(super) path: &'static str,
    pub(super) mtime: i64,
    pub(super) btime: Option<i64>,
    pub(super) content: Vec<u8>,
    pub(super) derived_from: Option<DerivedFrom>,
    pub(super) mime: Option<&'static str>,
}

impl EntryPlan {
    pub(super) fn new(path: &'static str, mtime: i64, content: Vec<u8>) -> Self {
        Self {
            path,
            mtime,
            btime: None,
            content,
            derived_from: None,
            mime: None,
        }
    }

    /// The birth time the writer's platform reported for this Entry's file
    /// (FM-9).
    pub(super) fn btime(mut self, btime: i64) -> Self {
        self.btime = Some(btime);
        self
    }

    pub(super) fn derived_from(mut self, container_id: ContainerId, path: &str) -> Self {
        self.derived_from = Some(DerivedFrom {
            container_id,
            path: EntryPath::nfc(path.to_owned()),
        });
        self
    }

    pub(super) fn mime(mut self, mime: &'static str) -> Self {
        self.mime = Some(mime);
        self
    }
}
