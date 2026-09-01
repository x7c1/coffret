use coffret_device::EntryPath;

/// What a browser can do with one Entry, decided from its name.
///
/// One table, in one place, and every route that has an opinion about a media
/// type asks it: the listing, so the explorer knows which rows it may open, and
/// the file route, so what it serves arrives as the type the listing promised.
/// Two tables would be two answers about one Entry, and the one that would go
/// wrong is the second.
///
/// The extension is the whole of the evidence. The Index has a `mime` column and
/// it is `None` for every row today — nothing fills it, because a scan reads a
/// file's bytes to hash and encrypt them rather than to identify them. Filling
/// it would not move the decision either: a stored `mime` is a creation-time
/// hint that no reader treats as a verdict (FM-9), so the table below decides
/// openability and nothing else does.
///
/// Adding a format is one line of [`OPENABLE`]. Nothing else in this crate knows
/// a media type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Media {
    /// Whether the explorer can display the Entry itself.
    pub openable: bool,
    /// What the bytes are served as.
    pub content_type: &'static str,
}

/// The formats a browser displays, by the extension a name ends in.
///
/// Kept to what every current browser renders from a plain `<img>`: an explorer
/// that offered a format the browser cannot draw would show a broken image
/// rather than a file it declines to open, which is the worse of the two.
const OPENABLE: [(&str, &str); 6] = [
    ("avif", "image/avif"),
    ("gif", "image/gif"),
    ("jpeg", "image/jpeg"),
    ("jpg", "image/jpeg"),
    ("png", "image/png"),
    ("webp", "image/webp"),
];

/// What everything else is served as.
///
/// Bytes with no claim about them: the browser is told the server is not saying
/// what they are, rather than being handed a guess it would then render.
const OPAQUE: &str = "application/octet-stream";

/// What the Entry at one path is, as far as a browser is concerned.
pub fn classify(path: &EntryPath) -> Media {
    match drawn_from(path) {
        Some(content_type) => Media {
            openable: true,
            content_type,
        },
        None => Media {
            openable: false,
            content_type: OPAQUE,
        },
    }
}

/// The media type a browser would draw the Entry at one path from, if any.
///
/// The extension is matched case-insensitively: `.JPG` and `.jpg` are one
/// person's two ways of naming the same kind of file.
fn drawn_from(path: &EntryPath) -> Option<&'static str> {
    let extension = extension_of(path.as_str())?.to_ascii_lowercase();
    OPENABLE
        .iter()
        .find(|(named, _)| *named == extension)
        .map(|(_, content_type)| *content_type)
}

/// What one Entry Path's last component ends in, after the last `.` in it.
///
/// A name that is all extension — `.gitignore` — has none: the dot there begins
/// the name rather than separating a suffix from it, and reading it as one would
/// make a hidden file's kind depend on what somebody happened to call it.
fn extension_of(path: &str) -> Option<&str> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let (stem, extension) = name.rsplit_once('.')?;
    (!stem.is_empty()).then_some(extension)
}

#[cfg(test)]
mod tests {
    use super::{classify, extension_of};
    use coffret_device::EntryPath;

    #[test]
    fn a_browser_image_is_openable_as_its_own_type() {
        for (path, content_type) in [
            ("albums/spring.jpg", "image/jpeg"),
            ("albums/spring.JPEG", "image/jpeg"),
            ("albums/cover.png", "image/png"),
            ("a.webp", "image/webp"),
            ("a.gif", "image/gif"),
            ("a.avif", "image/avif"),
        ] {
            let media = classify(&EntryPath::nfc(path));
            assert!(media.openable, "{path} is one a browser draws");
            assert_eq!(media.content_type, content_type);
        }
    }

    // Everything else is bytes the server makes no claim about. A format the
    // explorer cannot draw is not a failure and not an error — it is a row that
    // can be listed, downloaded, and not opened.
    #[test]
    fn everything_else_is_bytes_with_no_claim_about_them() {
        for path in [
            "notes.txt",
            "books/some-novel.pdf",
            "albums/raw/DSC_0001.NEF",
            "no-extension",
            ".gitignore",
            "albums/.hidden",
        ] {
            let media = classify(&EntryPath::nfc(path));
            assert!(!media.openable, "{path} is not one a browser draws");
            assert_eq!(media.content_type, "application/octet-stream");
        }
    }

    // A dot in a folder name says nothing about the file inside it.
    #[test]
    fn only_the_last_component_carries_the_extension() {
        assert_eq!(extension_of("albums.2026/readme"), None);
        assert_eq!(extension_of("albums.2026/cover.png"), Some("png"));
    }
}
