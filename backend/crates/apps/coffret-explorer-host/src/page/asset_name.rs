/// The name `path` stands for inside the build, or `None` where it names a
/// dotfile or steps outside it.
///
/// Percent-escapes are decoded first, so that `%2e%2e` is the `..` it spells,
/// and a segment that is empty, `.` or `..`, that starts with a dot, or that
/// holds a backslash, a NUL or an escaped slash is not a name. The root is
/// the empty name.
pub(super) fn asset_name(path: &str) -> Option<String> {
    let relative = path.strip_prefix('/')?;
    if relative.is_empty() {
        return Some(String::new());
    }
    let mut segments = Vec::new();
    for raw in relative.split('/') {
        let segment = decode(raw)?;
        let named = !segment.is_empty()
            && !segment.starts_with('.')
            && !segment.contains(['/', '\\', '\0']);
        if !named {
            return None;
        }
        segments.push(segment);
    }
    Some(segments.join("/"))
}

/// One segment with its percent-escapes decoded, or `None` where an escape is
/// broken or what it decodes to is not UTF-8.
fn decode(segment: &str) -> Option<String> {
    let bytes = segment.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%' {
            let hex = bytes.get(at + 1..at + 3)?;
            let hex = std::str::from_utf8(hex).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            at += 3;
        } else {
            decoded.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(decoded).ok()
}
