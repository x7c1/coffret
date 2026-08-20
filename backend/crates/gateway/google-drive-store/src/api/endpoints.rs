/// Where the Drive metadata API lives.
pub const DRIVE_API: &str = "https://www.googleapis.com/drive/v3";

/// Where uploads are sent, which is a different host path from the rest of the
/// API.
pub const DRIVE_UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3/files";

/// The two base URLs the gateway builds every call from.
///
/// A value rather than constants spread through the call sites, so that where
/// the gateway talks to is stated once and every URL is built the same way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoints {
    api: String,
    upload: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            api: DRIVE_API.to_owned(),
            upload: DRIVE_UPLOAD.to_owned(),
        }
    }
}

impl Endpoints {
    /// The URL of the file collection.
    pub fn files(&self) -> String {
        format!("{}/files", self.api)
    }

    /// The URL of one file.
    pub fn file(&self, id: &str) -> String {
        format!("{}/files/{}", self.api, percent_encode(id))
    }

    /// The URL uploads are opened against.
    pub fn upload(&self) -> &str {
        &self.upload
    }
}

/// Percent-encodes a path segment.
///
/// Drive's file ids are already URL-safe, but they come from the provider
/// rather than from coffret, so they are encoded rather than trusted to stay
/// that way.
fn percent_encode(segment: &str) -> String {
    let mut encoded = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_id_cannot_escape_its_path_segment() {
        let endpoints = Endpoints::default();
        assert_eq!(
            endpoints.file("../files/other"),
            format!("{DRIVE_API}/files/..%2Ffiles%2Fother")
        );
    }

    #[test]
    fn an_ordinary_file_id_is_left_as_it_is() {
        let endpoints = Endpoints::default();
        assert_eq!(
            endpoints.file("1a2B3c-_4d"),
            format!("{DRIVE_API}/files/1a2B3c-_4d")
        );
    }
}
