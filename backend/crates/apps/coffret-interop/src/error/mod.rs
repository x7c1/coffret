//! Everything that can stop the exchange, or a dump.
//!
//! The type and its variants are here; what each variant says is in
//! [`display`], and what it says underneath in [`source`].
//!
//! Every variant is one kind of failure the crate tells apart: a file that
//! could not be read or written, a manifest that states something this side
//! cannot take, an object the format crate would not open, a value that decoded
//! to something other than what the manifest states, and a dump that has no
//! key to open an object with. What a lower layer reported travels as the value
//! it reported — the operating system's `io::Error`, the format crate's or the
//! model's own refusal, the JSON or CBOR decoder's — so a caller printing the
//! chain reads that layer's answer under this one's rather than a copy of it
//! made here.
//!
//! Three variants say where rather than what: [`Error::InSet`],
//! [`Error::InFixture`] and [`Error::InEntry`] wrap a failure in the producer
//! of the set, the fixture and the Entry it was met at, so a failing exchange
//! points at the disagreement rather than at the exchange.

use std::io;
use std::path::PathBuf;
use std::str::Utf8Error;

use ciborium::Value;
use coffret_model::ContainerId;

mod display;

mod fixture_kind;
pub use fixture_kind::FixtureKind;

mod hex_defect;
pub use hex_defect::HexDefect;

mod source;

mod step;
pub use step::Step;

#[cfg(test)]
mod tests;

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can stop the exchange, or a dump.
#[derive(Debug)]
pub enum Error {
    /// A file could not be read: an object or a Recovery Code handed to a
    /// dump, a Keyring replica beside it, a manifest, or a fixture one points
    /// at.
    Read {
        /// The file that was being read.
        path: PathBuf,
        /// What the operating system reported.
        cause: io::Error,
    },
    /// A file of a fixture set being written could not be written.
    Write {
        /// The file that was being written.
        path: PathBuf,
        /// What the operating system reported.
        cause: io::Error,
    },
    /// A directory of a fixture set being written could not be created.
    CreateDirectory {
        /// The directory that was being created.
        path: PathBuf,
        /// What the operating system reported.
        cause: io::Error,
    },
    /// A manifest points at a file outside the set it describes.
    OutsideSet {
        /// The path as the manifest spells it.
        relative: String,
    },
    /// A manifest is not the JSON of the manifest schema.
    ManifestUnparsed {
        /// The manifest's file.
        path: PathBuf,
        /// What the JSON decoder reported.
        cause: serde_json::Error,
    },
    /// The manifest of a set being written could not be serialized.
    ManifestUnserialized {
        /// What the JSON encoder reported.
        cause: serde_json::Error,
    },
    /// What a dump opened could not be rendered as JSON.
    DumpUnrendered {
        /// What the JSON encoder reported.
        cause: serde_json::Error,
    },
    /// A manifest of a schema this build does not read.
    UnknownSchema {
        /// The schema the manifest states.
        found: u64,
    },
    /// A manifest that does not list a fixture every set carries.
    FixtureUnlisted {
        /// Which kind of fixture is missing.
        kind: FixtureKind,
        /// The name it is known by across both implementations.
        name: &'static str,
    },
    /// A value a manifest states in hex is not a hex spelling of it.
    StatedHex {
        /// Which value: the manifest's field name, or, inside a payload body,
        /// its path from the body down.
        field: String,
        /// What is wrong with the spelling.
        cause: HexDefect,
    },
    /// A value a manifest states is not one the model takes.
    StatedValue {
        /// Which value, by the manifest's field name.
        field: &'static str,
        /// What the model reported.
        cause: coffret_model::Error,
    },
    /// The format crate refused a step of writing or opening an object.
    Format {
        /// Which step it refused.
        step: Step,
        /// What the format crate reported.
        cause: coffret_format::Error,
    },
    /// The model refused a value a step was building.
    Model {
        /// Which step it refused.
        step: Step,
        /// What the model reported.
        cause: coffret_model::Error,
    },
    /// A value decoded to something other than what the manifest states.
    Differs {
        /// Which value: the manifest's field name, or, inside a payload body,
        /// its path from the body down.
        field: String,
        /// What it decoded to.
        decoded: String,
        /// What the manifest states.
        stated: String,
    },
    /// A Keyring's key table digests to something other than the `set_digest`
    /// its name carries (spec: FM-12, FM-17, KL-1).
    SetDigestDiffers {
        /// The digest of the key table this side decoded.
        computed: String,
        /// The digest the replica's name states.
        stated: String,
    },
    /// A Keyring replica is stored under a name that carries no `set_digest`.
    NoSetDigest,
    /// A control payload the generator built names an epoch other than the
    /// one the set is written under.
    EpochDiffers {
        /// The epoch the payload names.
        payload: u64,
        /// The epoch the set is written under.
        set: u64,
    },
    /// A Recovery Code fixture whose bytes are not text.
    RecoveryCodeNotText {
        /// Where the decoding failed.
        cause: Utf8Error,
    },
    /// A control payload's body is not readable CBOR.
    BodyUnreadable {
        /// What the CBOR decoder reported.
        cause: ciborium::de::Error<io::Error>,
    },
    /// Part of a payload body that the manifest states as a map is not one.
    NotAMap {
        /// Which part, as a path from the body down.
        what: String,
    },
    /// A map in a payload body carries another number of fields than the
    /// manifest states.
    FieldCount {
        /// Which map, as a path from the body down.
        what: String,
        /// How many fields it carries.
        found: usize,
        /// How many the manifest states.
        stated: usize,
    },
    /// A map in a payload body carries no field the manifest states.
    NoField {
        /// Which map, as a path from the body down.
        what: String,
        /// The field it does not carry.
        key: String,
    },
    /// Part of a payload body that the manifest states as an array is not
    /// one.
    NotAnArray {
        /// Which part, as a path from the body down.
        what: String,
    },
    /// An array in a payload body holds another number of elements than the
    /// manifest states.
    ElementCount {
        /// Which array, as a path from the body down.
        what: String,
        /// How many elements it holds.
        found: usize,
        /// How many the manifest states.
        stated: usize,
    },
    /// No control-object kind's key opens the object handed to a dump under
    /// the name it was handed with, and its header names no Container stored
    /// under that name either.
    NoKeyOpens {
        /// The name the object was handed with.
        object_name: String,
    },
    /// A dump was handed a Container and no Keyring replica to take its key
    /// from.
    ContainerNeedsKeyring {
        /// The name the Container is stored under.
        object_name: String,
    },
    /// The Keyring replica handed to a dump is saved under a name that is not
    /// Unicode, so it cannot be its object name.
    KeyringNameNotUnicode {
        /// Where the replica was handed from.
        path: PathBuf,
    },
    /// The Keyring replica handed to a dump does not open under the name it
    /// is saved as.
    KeyringUnopened {
        /// The name it is saved as.
        name: String,
        /// What the format crate reported.
        cause: coffret_format::Error,
    },
    /// The Keyring holds nothing for the Container a dump was handed.
    NothingForContainer {
        /// The Container.
        container_id: ContainerId,
    },
    /// The Keyring records the key of the Container a dump was handed as lost
    /// (spec: KL-7).
    ContainerKeyLost {
        /// The Container.
        container_id: ContainerId,
    },
    /// An object handed to a dump is shorter than a Container header.
    ShorterThanHeader {
        /// How long it is.
        len: usize,
    },
    /// A Container handed to a dump is shorter than the meta section its
    /// header declares.
    ShorterThanMetaSection {
        /// How long it is.
        len: usize,
        /// How long its header declares the header and meta section to be.
        declared: u64,
    },
    /// A payload map key a dump cannot name: neither text nor an integer.
    UnnamableKey {
        /// The key.
        found: Value,
    },
    /// A payload value a dump has no JSON rendering for.
    UnrenderableValue {
        /// The value.
        found: Value,
    },
    /// A failure met in a fixture set, by the implementation that wrote it.
    ///
    /// Both directions of the exchange are verified by the same code, so a
    /// failure says which side wrote the set it stopped on.
    InSet {
        /// Which implementation wrote the set, as its manifest states.
        producer: String,
        /// The failure.
        cause: Box<Error>,
    },
    /// A failure met writing or checking one fixture.
    InFixture {
        /// Which kind of fixture.
        kind: FixtureKind,
        /// The name it is known by across both implementations.
        fixture: String,
        /// The failure.
        cause: Box<Error>,
    },
    /// A failure met checking one Entry of a Container fixture.
    InEntry {
        /// The Entry's place in the Container's entry table.
        index: usize,
        /// The failure.
        cause: Box<Error>,
    },
}

impl Error {
    /// This failure, as met writing or checking the fixture `fixture`.
    pub(crate) fn in_fixture(self, kind: FixtureKind, fixture: &str) -> Self {
        Self::InFixture {
            kind,
            fixture: fixture.to_owned(),
            cause: Box::new(self),
        }
    }

    /// What a step the format crate refused is reported as, for `map_err`.
    pub(crate) fn format(step: Step) -> impl FnOnce(coffret_format::Error) -> Self {
        move |cause| Self::Format { step, cause }
    }

    /// What a step the model refused is reported as, for `map_err`.
    pub(crate) fn model(step: Step) -> impl FnOnce(coffret_model::Error) -> Self {
        move |cause| Self::Model { step, cause }
    }

    /// What a value the manifest states and the model refuses is reported as,
    /// for `map_err`.
    pub(crate) fn stated(field: &'static str) -> impl FnOnce(coffret_model::Error) -> Self {
        move |cause| Self::StatedValue { field, cause }
    }

    /// What a hex spelling the manifest states and that spells nothing is
    /// reported as, for `map_err`.
    pub(crate) fn stated_hex(field: impl Into<String>) -> impl FnOnce(HexDefect) -> Self {
        let field = field.into();
        move |cause| Self::StatedHex { field, cause }
    }

    /// A file that could not be read, for `map_err`.
    pub(crate) fn read(path: impl Into<PathBuf>) -> impl FnOnce(io::Error) -> Self {
        let path = path.into();
        move |cause| Self::Read { path, cause }
    }

    /// A file that could not be written, for `map_err`.
    pub(crate) fn write(path: impl Into<PathBuf>) -> impl FnOnce(io::Error) -> Self {
        let path = path.into();
        move |cause| Self::Write { path, cause }
    }
}
