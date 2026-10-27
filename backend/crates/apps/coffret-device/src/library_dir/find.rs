use coffret_model::{LibraryId, Redacted};
use tracing::warn;

use super::{entries, libraries_root, LibraryDir, STAGING_SUFFIX};
use crate::device_settings::DeviceSettings;
use crate::error::{Error, Result};
use crate::library_on_device::LibraryOnDevice;
use crate::library_ref::LibraryRef;

impl LibraryDir {
    /// The whole Library on this device that `which` names.
    ///
    /// An ID names the Library whose settings carry it, wherever its directory
    /// is. A name is matched against the copy of each Library's name the
    /// settings beside it keep, and against the directory's own name where an
    /// earlier build named the directory after the Library and nothing else
    /// yet records a name — or, once the directory has been moved to its ID
    /// before the Library was given that name, against the earlier name the
    /// settings keep — so a Library is found by the same name before and after
    /// it is moved to its ID.
    ///
    /// # Errors
    ///
    /// [`Error::NoSuchLibrary`] where no Library here answers to it, and
    /// [`Error::AmbiguousLibraryName`] where more than one carries the name:
    /// one of them is never chosen for the caller, and their IDs are what
    /// tells them apart.
    pub fn find(which: &LibraryRef) -> Result<Self> {
        let listed = listed()?;
        let found: Vec<&Listed> = match which {
            LibraryRef::Named(name) => listed
                .iter()
                .filter(|library| library.name.as_deref() == Some(name.as_str()))
                .collect(),
            // The directory named after its ID first, where an interrupted move
            // left one under both names.
            LibraryRef::Id(id) => {
                let mut held: Vec<&Listed> = listed
                    .iter()
                    .filter(|library| library.id == Some(*id))
                    .collect();
                held.sort_by_key(|library| !library.under_its_id());
                held.truncate(1);
                held
            }
        };
        match found.as_slice() {
            [one] => Ok(one.dir.clone()),
            [] => Err(Error::NoSuchLibrary {
                name: which.to_string(),
                path: libraries_root()?,
            }),
            several => Err(Error::AmbiguousLibraryName {
                name: which.to_string(),
                libraries: several.iter().filter_map(|library| library.id).collect(),
            }),
        }
    }

    /// Every whole Library on this device.
    ///
    /// Whole as [`is_present`](Self::is_present) means it: a directory still
    /// being built under the staging suffix is somebody's `init` or `join` in
    /// progress (or one that was interrupted), and a directory with no settings
    /// in it is what an interrupted removal leaves — neither is a Library
    /// anything can be opened from, so neither is offered. No `libraries/`
    /// directory at all is a device that has none yet, which is an empty list
    /// rather than a refusal.
    pub fn on_this_device() -> Result<Vec<Self>> {
        Ok(listed()?.into_iter().map(|library| library.dir).collect())
    }

    /// Every whole Library on this device, with its ID and the name it is
    /// listed under, for a person choosing one.
    ///
    /// The name is the copy the Library's settings keep, or — for a Library an
    /// earlier build put here and nothing has named yet — the name of the
    /// directory that build kept it in. A Library
    /// whose settings cannot be read is left out: nothing about it can be
    /// opened until they can.
    pub fn libraries() -> Result<Vec<LibraryOnDevice>> {
        Ok(listed()?
            .into_iter()
            .filter_map(|library| {
                Some(LibraryOnDevice {
                    library_id: library.id.filter(|_| library.readable)?,
                    name: library.name,
                })
            })
            .collect())
    }
}

#[cfg(test)]
impl LibraryDir {
    /// The Library on this device carrying `name`, which a case created under
    /// that name.
    pub(crate) fn named_here(name: &str) -> Self {
        Self::find(&LibraryRef::named(name))
            .unwrap_or_else(|refused| panic!("a case's Library {name:?} must be here: {refused}"))
    }
}

/// One whole Library directory, and what its settings say of it.
struct Listed {
    dir: LibraryDir,
    /// The ID its settings carry, or the one its directory is named after
    /// where the settings cannot be read.
    id: Option<LibraryId>,
    /// What it answers to by name; see [`LibraryDir::find`].
    name: Option<String>,
    /// Whether its settings read.
    readable: bool,
}

impl Listed {
    /// Whether the directory is the one its ID names.
    fn under_its_id(&self) -> bool {
        self.id
            .is_some_and(|id| self.dir.earlier_name(id).is_none())
    }
}

/// Every whole Library directory on this device, by directory name.
///
/// A settings file that cannot be read leaves the directory listed under what
/// its own name says, so that one named after a Library by an earlier build is
/// still found by that name and the refusal comes from reading the file.
fn listed() -> Result<Vec<Listed>> {
    let root = libraries_root()?;
    let mut found = Vec::new();
    for entry in entries(&root)? {
        if entry.ends_with(STAGING_SUFFIX) {
            continue;
        }
        let dir = LibraryDir {
            path: root.join(&entry),
            name: entry.clone(),
        };
        if !dir.is_present() {
            continue;
        }
        let listed = match DeviceSettings::read(&dir) {
            Ok(settings) => {
                let name = settings
                    .name
                    .clone()
                    .or_else(|| dir.earlier_name(settings.library_id).map(str::to_owned))
                    .or_else(|| settings.earlier_name.clone());
                Listed {
                    dir: dir.called(name.clone().unwrap_or_else(|| settings.library_id.to_hex())),
                    id: Some(settings.library_id),
                    name,
                    readable: true,
                }
            }
            Err(refused) => {
                warn!(
                    reason = %refused.redacted(),
                    "the settings of a Library on this device could not be read"
                );
                let id = LibraryId::from_hex(&entry).ok();
                Listed {
                    name: id.is_none().then(|| entry.clone()),
                    dir,
                    id,
                    readable: false,
                }
            }
        };
        found.push(listed);
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::isolated;
    use super::super::tests::library_id;
    use super::*;
    use crate::device_settings::ProviderSettings;

    /// Puts settings for the Library `seed` in `libraries/<directory>`, naming
    /// it `name` where one is given, the way a Library this build or an
    /// earlier one put there looks.
    fn plant(device: &isolated::Device, directory: &str, seed: u8, name: Option<&str>) {
        let dir = LibraryDir {
            name: directory.to_owned(),
            path: device.path().join("libraries").join(directory),
        };
        fs::create_dir_all(dir.path()).unwrap();
        DeviceSettings::new(
            library_id(seed),
            ProviderSettings::S3 {
                bucket: "photos".to_owned(),
                prefix: format!("coffret-{}/", library_id(seed).to_hex()),
                endpoint: None,
                region: None,
                path_style: false,
            },
        )
        .named(name.map(str::to_owned))
        .write(&dir)
        .unwrap();
    }

    // Only whole Libraries are offered: one still being built under the
    // staging suffix, and a directory an interrupted removal left with no
    // settings in it, are not anything a Library can be opened from.
    #[test]
    fn only_whole_libraries_are_on_this_device() {
        let device = isolated::device();
        assert!(
            LibraryDir::on_this_device()
                .expect("a device with no Libraries must list none")
                .is_empty(),
            "a device with no libraries directory has no Libraries"
        );

        plant(&device, &library_id(1).to_hex(), 1, Some("books"));
        plant(&device, "albums", 2, None);
        plant(&device, "drafts.partial", 3, Some("drafts"));
        fs::create_dir_all(device.path().join("libraries").join("emptied")).unwrap();

        let names: Vec<String> = LibraryDir::on_this_device()
            .expect("the libraries directory must be listable")
            .iter()
            .map(|dir| dir.name().to_owned())
            .collect();
        assert_eq!(names, ["books", "albums"]);
        assert_eq!(
            LibraryDir::libraries().expect("the libraries directory must be listable"),
            [
                LibraryOnDevice {
                    library_id: library_id(1),
                    name: Some("books".to_owned()),
                },
                LibraryOnDevice {
                    library_id: library_id(2),
                    name: Some("albums".to_owned()),
                },
            ]
        );
    }

    // A Library is found by its name and by its ID, in the directory its ID
    // names and in the one an earlier build named after it alike.
    #[test]
    fn a_library_is_found_by_its_name_or_its_id() {
        let device = isolated::device();
        plant(&device, &library_id(1).to_hex(), 1, Some("books"));
        plant(&device, "albums", 2, None);

        let books = LibraryDir::find(&LibraryRef::named("books")).expect("books is here");
        assert_eq!(books.name(), "books");
        assert_eq!(
            books,
            LibraryDir::find(&LibraryRef::Id(library_id(1))).expect("so is its ID")
        );
        let albums = LibraryDir::find(&LibraryRef::named("albums")).expect("albums is here");
        assert_eq!(
            albums.path(),
            device.path().join("libraries").join("albums")
        );
        assert_eq!(albums.earlier_name(library_id(2)), Some("albums"));
        assert_eq!(
            albums,
            LibraryDir::find(&LibraryRef::Id(library_id(2))).expect("so is its ID")
        );

        // A name spelling an ID is a name, and finds nothing here.
        let spelled = LibraryRef::named(library_id(1).to_hex());
        assert!(matches!(
            LibraryDir::find(&spelled),
            Err(Error::NoSuchLibrary { .. })
        ));
        assert!(matches!(
            LibraryDir::find(&LibraryRef::Id(library_id(9))),
            Err(Error::NoSuchLibrary { .. })
        ));
    }

    // Two Libraries carrying one name are not chosen between: the refusal names
    // both IDs, which is what tells them apart.
    #[test]
    fn a_name_two_libraries_carry_is_refused_with_both_ids() {
        let device = isolated::device();
        plant(&device, &library_id(1).to_hex(), 1, Some("books"));
        plant(&device, &library_id(2).to_hex(), 2, Some("books"));

        match LibraryDir::find(&LibraryRef::named("books")) {
            Err(Error::AmbiguousLibraryName { name, libraries }) => {
                assert_eq!(name, "books");
                assert_eq!(libraries, [library_id(1), library_id(2)]);
            }
            other => panic!("a name two Libraries carry must be refused, got {other:?}"),
        }
        assert!(LibraryDir::find(&LibraryRef::Id(library_id(2))).is_ok());
    }
}
