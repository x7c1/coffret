use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use coffret_model::{EntryPath, Redacted};
use tracing::warn;

use crate::folder_entry::FolderEntry;
use crate::folder_entry_kind::FolderEntryKind;
use crate::local_io_error::LocalIoError;
use crate::mapped_relative_location::MappedRelativeLocation;
use crate::mapped_roots::MappedRoots;
use crate::reserved_names::reserved_name;

use super::{DropWalk, DroppedItem, DroppedSource, NotTaken, NotTakenReason};

/// Walks what was dropped.
///
/// The items in the order given, and under a dropped folder its children in
/// the byte order of their names, each folder walked before its next sibling,
/// so the files of one folder stay together. An empty folder contributes
/// nothing, and is no refusal: a Library has no folders to add, only Entry
/// Paths whose separators imply them (spec: EP-2).
///
/// Each folder an item stands in is listed once, however many items of the
/// drop stand in it, so a drop of hundreds of photographs out of one folder is
/// one listing of it rather than hundreds.
///
/// Nothing here fails the drop as a whole. A folder that could not be read is
/// reported and the rest is walked, because the person let go of all of it
/// and the rest is still theirs to add.
pub async fn walk_dropped(roots: &dyn MappedRoots, items: &[DroppedItem]) -> DropWalk {
    let mut walk = DropWalk::default();
    // The answer for each folder an item stands in: its children, or why
    // there are none to look among.
    let mut listed: BTreeMap<PathBuf, Result<Vec<FolderEntry>, NotTakenReason>> = BTreeMap::new();
    for item in items {
        let label = item.name.to_string_lossy().into_owned();
        if !listed.contains_key(&item.parent) {
            let children = match roots.list_folder(&item.parent, None).await {
                Ok(Some(children)) => Ok(children),
                Ok(None) => Err(NotTakenReason::Missing),
                Err(cause) => Err(unreadable(&cause)),
            };
            listed.insert(item.parent.clone(), children);
        }
        let children = match &listed[&item.parent] {
            Ok(children) => children,
            Err(reason) => {
                walk.not_taken.push(NotTaken {
                    path: label,
                    reason: *reason,
                });
                continue;
            }
        };
        let Some(entry) = named(children, &item.name) else {
            walk.not_taken.push(NotTaken {
                path: label,
                reason: NotTakenReason::Missing,
            });
            continue;
        };
        walk_item(roots, &item.parent, entry.clone(), &mut walk).await;
    }
    walk
}

/// The child of a folder a dropped item's name names.
///
/// The name as the shell handed it over first. Failing that, the child whose
/// name is the same text once both are composed to NFC: macOS hands over the
/// path of what was dropped decomposed, while a folder lists a name as it was
/// written, and a name like `ガ.jpg` written composed would otherwise be
/// reported as not there (spec: EP-1). The walk below it reads the child's own
/// spelling, so the page is told the name as the folder lists it.
fn named<'a>(children: &'a [FolderEntry], name: &OsStr) -> Option<&'a FolderEntry> {
    if let Some(same) = children.iter().find(|child| child.name == name) {
        return Some(same);
    }
    let wanted = EntryPath::parse(name.to_str()?).ok()?;
    children.iter().find(|child| {
        child
            .name
            .to_str()
            .and_then(|text| EntryPath::parse(text).ok())
            .is_some_and(|spelled| spelled == wanted)
    })
}

/// One dropped item and everything under it.
async fn walk_item(roots: &dyn MappedRoots, root: &Path, item: FolderEntry, walk: &mut DropWalk) {
    // Each folder still being walked: where it is, what it is called as text,
    // and the children of it not yet visited, in reverse so the next one is
    // the last.
    let mut stack: Vec<(MappedRelativeLocation, String, Vec<FolderEntry>)> = Vec::new();
    let mut next = Some((None::<(MappedRelativeLocation, String)>, item));
    loop {
        let (above, entry) = match next.take() {
            Some(found) => found,
            None => {
                let Some((location, text, children)) = stack.last_mut() else {
                    return;
                };
                match children.pop() {
                    Some(child) => (Some((location.clone(), text.clone())), child),
                    None => {
                        stack.pop();
                        continue;
                    }
                }
            }
        };
        let Some(name) = entry.name.to_str().map(str::to_owned) else {
            walk.not_taken.push(NotTaken {
                path: joined(above.as_ref(), &entry.name.to_string_lossy()),
                reason: NotTakenReason::NotText,
            });
            continue;
        };
        let path = joined(above.as_ref(), &name);
        if let Some(reserved) = reserved_name(&name) {
            walk.not_taken.push(NotTaken {
                path,
                reason: NotTakenReason::Reserved(reserved),
            });
            continue;
        }
        let location = match &above {
            None => MappedRelativeLocation::from_component(entry.name.clone()),
            Some((location, _)) => location.below_component(entry.name.clone()),
        };
        match entry.kind {
            FolderEntryKind::File { size, mtime, btime } => walk.taken.push(DroppedSource {
                root: root.to_path_buf(),
                relative: location,
                path,
                size,
                mtime,
                btime,
            }),
            FolderEntryKind::Folder => match roots.list_folder(root, Some(&location)).await {
                Ok(Some(mut children)) => {
                    children.sort_by(|one, other| other.name.cmp(&one.name));
                    stack.push((location, path, children));
                }
                Ok(None) => walk.not_taken.push(NotTaken {
                    path,
                    reason: NotTakenReason::Missing,
                }),
                Err(cause) => walk.not_taken.push(NotTaken {
                    path,
                    reason: unreadable(&cause),
                }),
            },
            // A symbolic link, or anything else that is neither: not followed,
            // and not taken (spec: EP-8).
            FolderEntryKind::Other => walk.not_taken.push(NotTaken {
                path,
                reason: NotTakenReason::NotAFileOrFolder,
            }),
        }
    }
}

/// A folder the operating system would not list, as the page is told of it.
///
/// The page is told one word, and the cause is not lost on the way: the event
/// keeps which operation refused and what sort of failure it was, and nothing
/// about the folder (spec: EL-1).
fn unreadable(cause: &LocalIoError) -> NotTakenReason {
    warn!(
        operation = "walk_drop",
        reason = %cause.redacted(),
        "a folder the walk of a drop met could not be listed",
    );
    NotTakenReason::Unreadable
}

/// `name` below the folder `above` names as text, or on its own at the top.
fn joined(above: Option<&(MappedRelativeLocation, String)>, name: &str) -> String {
    match above {
        None => name.to_owned(),
        Some((_, text)) => format!("{text}/{name}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::in_memory_fs::InMemoryFs;
    use crate::reserved_names::ReservedName;
    use crate::{root_marker, scratch};

    fn item(path: &str) -> DroppedItem {
        DroppedItem::at(Path::new(path)).expect("a case drops an item in a folder")
    }

    fn taken(walk: &DropWalk) -> Vec<&str> {
        walk.taken.iter().map(|file| file.path.as_str()).collect()
    }

    // EP-8: a folder is walked depth first, each folder before its next
    // sibling and in the byte order of the names, and its files are named
    // relative to the folder the dropped one stood in.
    #[tokio::test]
    async fn a_nested_folder_is_walked_depth_first() {
        let fs = InMemoryFs::new();
        for path in [
            "/drops/book/b.jpg",
            "/drops/book/a/2.jpg",
            "/drops/book/a/1.jpg",
            "/drops/book/c/x.jpg",
            "/drops/loose.jpg",
        ] {
            fs.write_file(Path::new(path), b"page");
        }

        let walk = walk_dropped(&fs, &[item("/drops/book"), item("/drops/loose.jpg")]).await;

        assert_eq!(
            taken(&walk),
            [
                "book/a/1.jpg",
                "book/a/2.jpg",
                "book/b.jpg",
                "book/c/x.jpg",
                "loose.jpg"
            ]
        );
        assert!(walk.not_taken.is_empty(), "{:?}", walk.not_taken);
        let first = &walk.taken[0];
        assert_eq!(first.root, Path::new("/drops"));
        assert_eq!(first.relative.to_path_buf(), Path::new("book/a/1.jpg"));
        assert_eq!(first.size, 4);
    }

    // EP-11, EP-14: a name coffret keeps for itself is reported, and nothing
    // under it is walked.
    #[tokio::test]
    async fn a_reserved_name_is_reported_and_not_walked() {
        let fs = InMemoryFs::new();
        let scratch = scratch::incoming_name();
        fs.write_file(
            &Path::new("/drops/book").join(scratch.as_str()),
            b"half a page",
        );
        fs.write_file(
            &Path::new("/drops/book")
                .join(root_marker::MANAGEMENT_AREA)
                .join(root_marker::MARKER_FILE),
            b"a marker",
        );
        fs.write_file(Path::new("/drops/book/page.jpg"), b"page");

        let walk = walk_dropped(&fs, &[item("/drops/book")]).await;

        assert_eq!(taken(&walk), ["book/page.jpg"]);
        let mut reasons: Vec<NotTakenReason> =
            walk.not_taken.iter().map(|one| one.reason).collect();
        reasons.sort_by_key(|reason| format!("{reason:?}"));
        assert_eq!(
            reasons,
            [
                NotTakenReason::Reserved(ReservedName::ManagementArea),
                NotTakenReason::Reserved(ReservedName::Scratch),
            ]
        );
    }

    // EP-1: a dropped item whose path the shell spelled decomposed is found
    // under the name its folder lists composed, and taken by that spelling.
    #[tokio::test]
    async fn an_item_spelled_decomposed_is_found_under_its_composed_name() {
        let fs = InMemoryFs::new();
        fs.write_file(Path::new("/drops/\u{30AC}.jpg"), b"page");
        fs.write_file(Path::new("/drops/\u{30AE}/1.jpg"), b"page");

        let walk = walk_dropped(
            &fs,
            &[
                item("/drops/\u{30AB}\u{3099}.jpg"),
                item("/drops/\u{30AD}\u{3099}"),
            ],
        )
        .await;

        assert!(walk.not_taken.is_empty(), "{:?}", walk.not_taken);
        assert_eq!(taken(&walk), ["\u{30AC}.jpg", "\u{30AE}/1.jpg"]);
    }

    // A path that names nothing there is reported, not dropped silently.
    #[tokio::test]
    async fn an_item_that_is_not_there_is_reported() {
        let fs = InMemoryFs::new();
        fs.write_file(Path::new("/drops/here.jpg"), b"here");

        let walk = walk_dropped(
            &fs,
            &[item("/drops/gone.jpg"), item("/elsewhere/also-gone.jpg")],
        )
        .await;

        assert!(walk.taken.is_empty());
        assert_eq!(
            walk.not_taken,
            [
                NotTaken {
                    path: "gone.jpg".to_owned(),
                    reason: NotTakenReason::Missing,
                },
                NotTaken {
                    path: "also-gone.jpg".to_owned(),
                    reason: NotTakenReason::Missing,
                },
            ]
        );
    }
}
