//! The folder with the most rows: the one whose listing is the largest.
//!
//! A listing shows a folder's children one level down — the files directly in
//! it and the folders directly under it — so the largest folder is the one
//! with the most of those, which is where the listing is timed besides the
//! root.

use std::collections::{HashMap, HashSet};

/// One folder and how many rows its listing has.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Largest {
    /// The folder, as an Entry Path; `None` is the Library root.
    pub folder: Option<String>,
    pub children: usize,
}

/// The folder with the most children among every folder `paths` imply, the
/// root included; of two with as many, the one whose path sorts first. `None`
/// where there are no paths.
pub fn largest_folder<'a>(paths: impl IntoIterator<Item = &'a str>) -> Option<Largest> {
    let mut children: HashMap<Option<&str>, HashSet<&str>> = HashMap::new();
    for path in paths {
        // Every folder above the Entry has the next segment down as a child.
        let mut parent: Option<&str> = None;
        let mut start = 0;
        loop {
            let end = path[start..].find('/').map(|at| start + at);
            let child = &path[start..end.unwrap_or(path.len())];
            children.entry(parent).or_default().insert(child);
            match end {
                Some(end) => {
                    parent = Some(&path[..end]);
                    start = end + 1;
                }
                None => break,
            }
        }
    }
    children
        .into_iter()
        .map(|(folder, names)| (folder, names.len()))
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|(folder, children)| Largest {
            folder: folder.map(str::to_owned),
            children,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_files_and_folders_one_level_down() {
        let paths = [
            "Comics/A/Vol. 01/001.jpg",
            "Comics/A/Vol. 01/002.jpg",
            "Comics/A/Vol. 02/001.jpg",
            "Comics/A/notes.txt",
            "Comics/B/Vol. 01/001.jpg",
        ];
        assert_eq!(
            largest_folder(paths),
            Some(Largest {
                folder: Some("Comics/A".to_owned()),
                children: 3,
            }),
            "two volumes and a file; Vol. 01's two pages are one row each under it, not here"
        );
    }

    #[test]
    fn the_root_can_be_the_largest() {
        let paths = ["a.jpg", "b.jpg", "C/001.jpg"];
        assert_eq!(
            largest_folder(paths),
            Some(Largest {
                folder: None,
                children: 3,
            })
        );
    }

    #[test]
    fn of_two_as_large_the_first_by_path() {
        let paths = [
            "B/1.jpg", "B/2.jpg", "B/3.jpg", "A/1.jpg", "A/2.jpg", "A/3.jpg",
        ];
        assert_eq!(
            largest_folder(paths),
            Some(Largest {
                folder: Some("A".to_owned()),
                children: 3,
            })
        );
        assert_eq!(largest_folder([]), None);
    }
}
