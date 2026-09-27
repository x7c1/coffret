//! The mapped folder on this device's disk, as a case plants into it and reads
//! it back.

use std::path::Path;

use super::{plant, Served};

impl Served {
    /// Puts a file into the mapped folder that this device did not place there.
    pub fn plant_locally(&self, path: &str, content: &[u8]) {
        plant(self.local.path(), path, content);
    }

    /// Replaces a local file name with a symbolic link to a test-owned path.
    #[cfg(unix)]
    pub fn replace_with_symlink(&self, path: &str, target: &Path) {
        let local = self.local_path(path);
        match std::fs::remove_file(&local) {
            Ok(()) => {}
            Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => {}
            Err(cause) => panic!("removing the old local name must succeed: {cause}"),
        }
        if let Some(parent) = local.parent() {
            std::fs::create_dir_all(parent).expect("the link's parent exists");
        }
        std::os::unix::fs::symlink(target, local).expect("making the local symbolic link");
    }

    /// Whether the mapped folder holds a file for one Entry Path.
    pub fn holds(&self, path: &str) -> bool {
        self.local_path(path).is_file()
    }

    /// Where in the mapped folder one Entry's file belongs (spec: EP-9).
    pub fn local_path(&self, path: &str) -> std::path::PathBuf {
        self.local.path().join(path)
    }

    /// Everything standing in one folder of the mapped folder, sorted.
    ///
    /// Names and not rows: what a case asking this is about is what a request
    /// left on disk, which includes the names no listing would ever show — the
    /// scratch of a transfer that stopped (spec: EP-11) least of all.
    pub fn folder_names(&self, folder: &str) -> Vec<String> {
        let mut names: Vec<String> = match std::fs::read_dir(self.local.path().join(folder)) {
            Ok(entries) => entries
                .map(|entry| {
                    entry
                        .expect("a folder of the mapped folder can be read")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect(),
            // A folder nothing was ever written into is not there at all, which
            // is the same answer as a folder holding nothing.
            Err(missing) if missing.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(cause) => panic!("a folder of the mapped folder can be read: {cause}"),
        };
        names.sort();
        names
    }
}
