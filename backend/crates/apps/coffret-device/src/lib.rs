//! What one device keeps for a Library, and how it opens one.
//!
//! Every flow coffret performs already exists as a library call — the sync, the
//! freeze, the fetch, the catalog, the two gateways, the byte forms and the
//! keys — and until this crate nothing composed them. That is what this is: the
//! composition root, the place where "a Library on this device" stops being an
//! argument list and becomes a directory with a Master Key, a catalog, a spool,
//! and a note of where its Storage is.
//!
//! It is a library and not a command for one reason. The browser-based explorer
//! will do the same things — open a Library, record a mapping, eventually
//! create one — and two implementations of "where a device keeps a Library"
//! would disagree the first time either changed. The command line is a shell
//! over this; so is the explorer.
//!
//! # One directory per Library
//!
//! Under the state directory the platform names — `$XDG_STATE_HOME`, or
//! `$HOME/.local/state` where that is unset:
//!
//! ```text
//! coffret/libraries/<name>/
//!   settings.json      where the Library lives (this crate's contract)
//!   master-key.cfmk    the Master Key under the Passphrase (spec: KD-9)
//!   token-cache.cftc   the sealed OAuth grant (spec: KD-10), Drive only
//!   index.sqlite       the catalog
//!   spool/             encrypted Containers waiting to be uploaded
//! ```
//!
//! `<name>` is what this device calls the Library, not what the Library calls
//! itself: another device holding the same Library may call it something else,
//! the way it may map its folders differently (spec: CK-7). The four files and
//! the directory are created owner-only, and none of them is named in
//! `settings.json` — the layout is the single answer to where each piece is.
//!
//! [`STATE_DIRECTORY`] stands in for the `coffret` directory itself rather than
//! for what is above it, so a Library of a run under it is at
//! `$COFFRET_STATE_DIR/libraries/<name>/` and not under a second `coffret`.
//!
//! # What the settings file is for
//!
//! [`DeviceSettings`] is a contract rather than a convenience: the explorer will
//! read the same file, so it is versioned, and a version this build does not
//! know is refused rather than repaired. It holds two things — which Library
//! this is, and where on Storage it is — and no credential a device could not
//! obtain again for itself.
//!
//! # The Passphrase, and what it does not reach
//!
//! [`create_library`] draws a Master Key, stores it under the Passphrase, and
//! hands back the [`RecoveryCode`] — the one copy of that key which exists off
//! the device. [`open_library`] spends the Passphrase once, derives what the
//! flows need, and drops the key when the process ends: one process is one
//! unlock (spec: DK-9).
//!
//! Recording a mapping needs no Passphrase at all, because a mapping is device
//! state in a plaintext catalog and says nothing the Library keeps secret
//! (spec: CK-7).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod authorize;
pub use authorize::authorize;

mod create_library;
pub use create_library::{create_library, CreateLibraryRequest, CreatedLibrary, NewProvider};

mod device_settings;
pub use device_settings::{DeviceSettings, ProviderSettings};

// The three things every Drive flow here is built from, kept in one place so
// that the cache one command writes is the cache the next one reads.
mod drive;

mod error;
pub use error::{CreationStep, Error, NameDefect, Result};

mod library_dir;
pub use library_dir::{LibraryDir, STATE_DIRECTORY};

mod mapping;
pub use mapping::{mappings, set_mapping};

// Whether a piece of text is one path component. Both the name a Library has on
// this device and a mapping's prefix are held to that shape, so the check
// belongs to neither of them.
mod name_defect;

mod open_library;
pub use open_library::{open_library, OpenLibrary};

// How a device's own files are created: owner-only, and through a rename, so an
// interrupted write leaves what was there rather than half of what replaces it.
mod owner_only;

mod recovery_code;
pub use recovery_code::recovery_code;

// What a shell over this crate needs to name and would otherwise have to reach
// past it for. The Recovery Code is a value `create_library` hands back and
// `recovery_code` produces, and a mapping is what `mappings` returns; neither
// belongs to this crate, and a caller printing either should not have to take a
// dependency on the layer that owns it.
pub use coffret_format::RecoveryCode;
pub use coffret_usecase::device_state::Mapping;

mod stored_master_key_file;
pub use stored_master_key_file::StoredMasterKeyFile;

// What this crate's own tests build a Library from, in one place so that the
// state directory the environment names is set once for the whole binary.
#[cfg(test)]
mod testing;
