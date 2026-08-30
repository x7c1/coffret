use super::DeviceSettings;
use crate::error::{Error, Result};
use crate::library_dir::LibraryDir;
use crate::owner_only;

impl DeviceSettings {
    /// Writes these settings as the settings of the Library in `dir`.
    ///
    /// Pretty-printed, because the file is a contract that a person reads and
    /// occasionally has to correct by hand, and owner-only, because for a Drive
    /// Library it carries the OAuth client secret.
    pub fn write(&self, dir: &LibraryDir) -> Result<()> {
        let path = dir.settings_file();
        let mut document =
            serde_json::to_vec_pretty(self).map_err(|cause| Error::UnencodableSettings {
                path: path.clone(),
                cause,
            })?;
        document.push(b'\n');

        owner_only::write_file("writing the settings", &path, &document)
    }
}
