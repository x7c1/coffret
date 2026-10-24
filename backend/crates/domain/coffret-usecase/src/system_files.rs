//! The files and folders an operating system writes beside a person's own.
//!
//! The Finder leaves `.DS_Store` in every folder it shows and `Icon\r` in one
//! given a custom icon, and writes AppleDouble `._` files beside the person's
//! own on a volume that keeps no extended attributes; unpacking a zip the
//! Finder made leaves a `__MACOSX` folder; Windows Explorer leaves `Thumbs.db`,
//! `ehthumbs.db` and `desktop.ini`, and KDE's file manager `.directory`. A
//! volume's top carries more of the same: `.Spotlight-V100`, `.fseventsd` and
//! `.TemporaryItems` on macOS, `$RECYCLE.BIN` and `System Volume Information`
//! on Windows. None of it is the person's, and once one is inside a Pack it
//! leaves only with the whole Pack rewritten (spec: PK-10), so no scan and no
//! drop takes one in (spec: EP-17).
//!
//! Everything but a volume's folders is left out at any depth. Those are left
//! out only as the first component directly under a mapped root, or directly
//! in a dropped folder, because that is the one place a system puts them; a folder
//! of the same name deeper down is the person's own. Exactly these spellings,
//! compared byte for byte, as [`trash_folders`](crate::trash_folders) compares
//! its own: a system writes them so, and a name that differs in case is one
//! somebody chose. A name of the person's own that happens to begin with `._`
//! is left out too, the cost EP-17 states.
//!
//! The list's source is `docs/spec/entry-path/system-files.json`. This module
//! declares it for the backend and `systemFiles.ts` in the explorer declares it
//! for a browser's drop, which is walked on the page before anything is sent;
//! each is held to that file by a test, so neither can drift from it alone.
//!
//! Nothing here does any I/O, for the reason
//! [`trash_folders`](crate::trash_folders) beside it does none: the scan, the
//! drop's walk and the upload ask one question of a name, and it has one
//! answer.

/// The names left out wherever they stand.
const ANYWHERE: [&str; 7] = [
    ".DS_Store",
    // The Finder's custom-folder-icon file: the four letters and a carriage
    // return.
    "Icon\r",
    "Thumbs.db",
    "ehthumbs.db",
    "desktop.ini",
    ".directory",
    "__MACOSX",
];

/// What a name left out wherever it stands begins with: AppleDouble's.
const ANYWHERE_PREFIXES: [&str; 1] = ["._"];

/// The names left out only as the first component under a mapped root, or
/// directly in a dropped folder.
const AT_TOP: [&str; 5] = [
    ".Spotlight-V100",
    ".fseventsd",
    ".TemporaryItems",
    "$RECYCLE.BIN",
    "System Volume Information",
];

/// Where a name stands, which is the half of the question the name cannot
/// answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Depth {
    /// The first component directly under a mapped root, or directly in a
    /// dropped folder.
    Top,
    /// Anywhere else: deeper down, or a dropped item itself.
    Below,
}

/// Whether a name standing at `depth` is one an operating system keeps rather
/// than the person's own (spec: EP-17).
///
/// The caller decides where the name stands: this answers only from the name
/// and that.
pub fn is_system_file(name: &str, depth: Depth) -> bool {
    ANYWHERE.contains(&name)
        || ANYWHERE_PREFIXES
            .iter()
            .any(|prefix| name.starts_with(prefix))
        || (depth == Depth::Top && AT_TOP.contains(&name))
}

/// The first of `components` — a path below a mapped root, its topmost
/// component first — that is one an operating system keeps, where one is
/// (spec: EP-17).
pub fn system_component<'a>(components: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    components.into_iter().enumerate().find_map(|(at, name)| {
        let depth = if at == 0 { Depth::Top } else { Depth::Below };
        is_system_file(name, depth).then_some(name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The registry the declarations are held to.
    fn registry() -> serde_json::Value {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../docs/spec/entry-path/system-files.json"
        );
        let text = std::fs::read_to_string(path).expect("the system-file registry is readable");
        serde_json::from_str(&text).expect("the system-file registry is JSON")
    }

    fn names(registry: &serde_json::Value, key: &str) -> Vec<String> {
        registry[key]
            .as_array()
            .unwrap_or_else(|| panic!("the registry lists {key}"))
            .iter()
            .map(|name| name.as_str().expect("a name is text").to_owned())
            .collect()
    }

    fn declared(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    // EP-17: the declarations are the registry's, each list in its order.
    #[test]
    fn the_declared_names_are_the_registrys() {
        let registry = registry();
        assert_eq!(names(&registry, "anywhere"), declared(&ANYWHERE));
        assert_eq!(
            names(&registry, "anywhere_prefixes"),
            declared(&ANYWHERE_PREFIXES)
        );
        assert_eq!(names(&registry, "top"), declared(&AT_TOP));
    }

    // EP-17: every name left out wherever it stands is left out at the top and
    // deeper, and so is anything AppleDouble's prefix begins.
    #[test]
    fn a_name_a_system_writes_anywhere_is_left_out_at_any_depth() {
        for name in [
            ".DS_Store",
            "Icon\r",
            "Thumbs.db",
            "ehthumbs.db",
            "desktop.ini",
            ".directory",
            "__MACOSX",
            "._page-001.jpg",
            "._",
            "._.DS_Store",
        ] {
            for depth in [Depth::Top, Depth::Below] {
                assert!(is_system_file(name, depth), "{name:?} at {depth:?}");
            }
        }
    }

    // EP-17: a volume's own folders are left out at the top alone; deeper down a
    // folder of that name is the person's.
    #[test]
    fn a_volumes_folder_is_left_out_only_at_the_top() {
        for name in [
            ".Spotlight-V100",
            ".fseventsd",
            ".TemporaryItems",
            "$RECYCLE.BIN",
            "System Volume Information",
        ] {
            assert!(is_system_file(name, Depth::Top), "{name:?} at the top");
            assert!(!is_system_file(name, Depth::Below), "{name:?} deeper down");
        }
    }

    // EP-17: exactly those spellings, so a name that merely resembles one is the
    // person's own, wherever it stands.
    #[test]
    fn a_name_that_only_resembles_one_is_the_persons_own() {
        for name in [
            ".ds_store",
            ".DS_STORE",
            "DS_Store",
            "Thumbs.DB",
            "thumbs.db",
            "Desktop.ini",
            "_MACOSX",
            "__macosx",
            "__MACOSX ",
            "Icon",
            "Icon\n",
            "icon\r",
            ".Spotlight-V100x",
            ".spotlight-v100",
            "$Recycle.Bin",
            "_page-001.jpg",
            ".page-001.jpg",
            "page._001.jpg",
            ".Trash",
        ] {
            for depth in [Depth::Top, Depth::Below] {
                assert!(!is_system_file(name, depth), "{name:?} at {depth:?}");
            }
        }
    }

    // EP-17: the component a path below a mapped root is left out for, the
    // volume's folders counted only as the first.
    #[test]
    fn a_path_is_left_out_for_its_first_system_component() {
        assert_eq!(system_component(["book", ".DS_Store"]), Some(".DS_Store"));
        assert_eq!(
            system_component(["__MACOSX", "book", "._1.jpg"]),
            Some("__MACOSX")
        );
        assert_eq!(
            system_component([".Spotlight-V100", "Store-V2"]),
            Some(".Spotlight-V100")
        );
        assert_eq!(system_component(["book", ".Spotlight-V100", "x"]), None);
        assert_eq!(system_component(["book", "page-001.jpg"]), None);
        assert_eq!(system_component([]), None);
    }
}
