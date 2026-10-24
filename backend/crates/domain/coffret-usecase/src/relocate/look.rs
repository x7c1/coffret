use coffret_model::EntryPath;

use crate::departure::{mapped_file, MappedFile};
use crate::device_state::{LocalEntry, LocalEntryState, Mapping};
use crate::fetch::{reaching, relative_below};
use crate::mapped_relative_location::MappedRelativeLocation;
use crate::unavailable_root::RootUnavailable;
use crate::{root_marker, scratch, system_files, trash_folders};

use super::{disk, RelocationRefused, Run};

/// One waiting record, and what stands on disk for it.
pub(super) enum Look {
    /// Both paths stand under one root that may be relocated in.
    Looked(Box<Looked>),
    /// The record cannot be acted on this run, for the reason given.
    Refused {
        local: LocalEntry,
        reason: RelocationRefused,
    },
}

pub(super) struct Looked {
    pub(super) local: LocalEntry,
    pub(super) mapping: Mapping,
    /// What stands where the file was placed. Not looked at for an absent
    /// record, which has no file.
    pub(super) source: MappedFile,
    /// What stands at the Entry's current path.
    pub(super) target: MappedFile,
    /// The Entry's current path below the root, as a placement spells it.
    pub(super) to_relative: MappedRelativeLocation,
}

impl Looked {
    pub(super) fn from(&self) -> &EntryPath {
        &self.local.observation.path
    }

    pub(super) fn to(&self) -> &EntryPath {
        self.local
            .relocate_to
            .as_ref()
            .expect("only a waiting record is looked at")
    }
}

impl Run<'_> {
    pub(super) async fn look_all(&mut self, waiting: Vec<LocalEntry>) -> Vec<Look> {
        let mut looked = Vec::with_capacity(waiting.len());
        for local in waiting {
            looked.push(self.look(local).await);
        }
        looked
    }

    /// Reads what stands for one waiting record, refusing where the record
    /// cannot be acted on at all.
    async fn look(&mut self, local: LocalEntry) -> Look {
        let refused = |local, reason| Look::Refused { local, reason };
        let from = &local.observation.path;
        let to = local
            .relocate_to
            .as_ref()
            .expect("only a waiting record is read back as one");
        let (Some(mapping), Some(mapping_to)) =
            (reaching(&self.mappings, from), reaching(&self.mappings, to))
        else {
            return refused(local, RelocationRefused::OutsideOneRoot);
        };
        if mapping.prefix != mapping_to.prefix {
            return refused(local, RelocationRefused::OutsideOneRoot);
        }
        let mapping = mapping.clone();
        let (Some(_), Some(to_below)) =
            (relative_below(&mapping, from), relative_below(&mapping, to))
        else {
            return refused(local, RelocationRefused::OutsideOneRoot);
        };
        if reserved(&to_below) {
            return refused(local, RelocationRefused::ReservedName);
        }
        if let Some(reason) = self.root_refusal(&mapping).await {
            return refused(local, reason);
        }
        let roots = self.disk.roots;
        let source = match local.state {
            LocalEntryState::Present => match mapped_file(roots, &mapping, from).await {
                Ok(source) => source,
                Err(refused_io) => return refused(local, disk(refused_io.into())),
            },
            LocalEntryState::Absent => MappedFile::Nothing,
        };
        let target = match mapped_file(roots, &mapping, to).await {
            Ok(target) => target,
            Err(refused_io) => return refused(local, disk(refused_io.into())),
        };
        Look::Looked(Box::new(Looked {
            local,
            mapping,
            source,
            target,
            to_relative: MappedRelativeLocation::from_entry_path(&to_below),
        }))
    }

    /// Why nothing may be relocated under `mapping`'s root this run, if
    /// anything stops it (spec: EP-12).
    ///
    /// Stricter than a scan's question, and deliberately: a root standing on
    /// another filesystem than the recorded one is refused whatever it holds.
    /// A scan re-stamps one that holds files, and the next run relocates then;
    /// relocating files on a disk this device has not yet recognized is not a
    /// question worth answering early.
    async fn root_refusal(&mut self, mapping: &Mapping) -> Option<RelocationRefused> {
        if let Some(known) = self.roots.get(&mapping.prefix) {
            return known.clone();
        }
        let refusal = match self.disk.roots.probe_root(&mapping.local_root).await {
            Err(refused_io) => Some(disk(refused_io.into())),
            Ok(None) => Some(RelocationRefused::Unavailable(RootUnavailable::Missing)),
            Ok(Some(probe)) => match (probe.identity, mapping.root_identity.as_ref()) {
                (Some(now), Some(recorded)) if now != *recorded => Some(
                    RelocationRefused::Unavailable(RootUnavailable::AnotherFilesystem),
                ),
                _ => None,
            },
        };
        self.roots.insert(mapping.prefix.clone(), refusal.clone());
        refusal
    }
}

/// Whether a path below a mapped root passes through a name the folder keeps
/// for something other than content: coffret's own, the desktop's trash, or
/// one an operating system keeps beside the person's files (spec: EP-11,
/// EP-14, EP-16, EP-17).
///
/// The last because no scan finds a file at such a name: a file relocated
/// there would leave its Entry with nothing this device ever looks at again.
fn reserved(below: &EntryPath) -> bool {
    let mut names = below.as_str().split('/');
    names
        .clone()
        .next()
        .is_some_and(trash_folders::is_trash_folder)
        || system_files::system_component(names.clone()).is_some()
        || names.any(|name| {
            root_marker::is_management_area(name)
                || root_marker::folds_to_management_area(name)
                || scratch::is_scratch(name)
        })
}

#[cfg(test)]
mod tests {
    use super::reserved;
    use crate::entry_paths::entry_path;

    // EP-17: a new path through a name an operating system keeps is refused
    // as the names coffret and the desktop keep are, the volume's folders only
    // as the first component below the root, as a scan leaves them out.
    #[test]
    fn a_new_path_through_a_system_files_name_is_reserved() {
        for below in [
            "book/.DS_Store",
            "._page.jpg",
            "__MACOSX/book/page.jpg",
            ".Spotlight-V100/x",
        ] {
            assert!(reserved(&entry_path(below)), "{below}");
        }
        for below in ["book/page.jpg", "book/.Spotlight-V100/x", "book/.ds_store"] {
            assert!(!reserved(&entry_path(below)), "{below}");
        }
    }
}
