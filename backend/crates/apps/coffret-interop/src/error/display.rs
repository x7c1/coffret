//! What each failure says.
//!
//! A failure carrying a cause says only its own part: the cause is the next
//! link of the chain, and a caller printing the chain reads it there, so each
//! thing is said once.

use std::fmt;

use super::Error;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read { path, .. } => write!(f, "reading {}", path.display()),
            Self::Write { path, .. } => write!(f, "writing {}", path.display()),
            Self::CreateDirectory { path, .. } => write!(f, "creating {}", path.display()),
            Self::OutsideSet { relative } => {
                write!(f, "{relative:?} is not a path inside the fixture set")
            }
            Self::ManifestUnparsed { path, .. } => write!(f, "parsing {}", path.display()),
            Self::ManifestUnserialized { .. } => f.write_str("serializing the manifest"),
            Self::DumpUnrendered { .. } => f.write_str("rendering the object"),
            Self::UnknownSchema { found } => write!(
                f,
                "manifest schema {found} is not the schema {} this build reads",
                crate::manifest::SCHEMA
            ),
            Self::FixtureUnlisted { kind, name } => {
                write!(f, "the manifest lists no {kind} fixture {name:?}")
            }
            Self::StatedHex { field, .. } => f.write_str(field),
            Self::StatedValue { field, .. } => f.write_str(field),
            Self::Format { step, .. } | Self::Model { step, .. } => write!(f, "{step}"),
            Self::Differs {
                field,
                decoded,
                stated,
            } => write!(
                f,
                "{field}: decoded {decoded}, the manifest states {stated}"
            ),
            Self::SetDigestDiffers { computed, stated } => write!(
                f,
                "set_digest: the key table digests to {computed:?}, the name states {stated:?}"
            ),
            Self::NoSetDigest => f.write_str("the Keyring replica's name carries no set_digest"),
            Self::EpochDiffers { payload, set } => write!(
                f,
                "the payload names epoch {payload}, the set is written under epoch {set}"
            ),
            Self::RecoveryCodeNotText { .. } => {
                f.write_str("a Recovery Code is text, and these bytes are not UTF-8")
            }
            Self::BodyUnreadable { .. } => f.write_str("the payload body is not readable CBOR"),
            Self::NotAMap { what } => write!(f, "{what} is not a CBOR map"),
            Self::FieldCount {
                what,
                found,
                stated,
            } => write!(
                f,
                "{what} carries {found} fields, the manifest states {stated}"
            ),
            Self::NoField { what, key } => write!(f, "{what} carries no field {key:?}"),
            Self::NotAnArray { what } => write!(f, "{what} is not an array"),
            Self::ElementCount {
                what,
                found,
                stated,
            } => write!(
                f,
                "{what} holds {found} elements, the manifest states {stated}"
            ),
            Self::NoKeyOpens { object_name } => write!(
                f,
                "no control-object kind's key opens this object under the name {object_name:?}"
            ),
            Self::ContainerNeedsKeyring { object_name } => write!(
                f,
                "{object_name} is a Container; pass --keyring to render its meta section"
            ),
            Self::KeyringNameNotUnicode { path } => write!(
                f,
                "the Keyring replica {} is saved under a name that is not Unicode",
                path.display()
            ),
            Self::KeyringUnopened { name, .. } => write!(
                f,
                "opening the Keyring replica; it has to be saved under its own object name, \
                 not {name:?}"
            ),
            Self::NothingForContainer { container_id } => write!(
                f,
                "the Keyring holds nothing for Container {}",
                container_id.to_hex()
            ),
            Self::ContainerKeyLost { container_id } => write!(
                f,
                "the Keyring records the key of Container {} as lost",
                container_id.to_hex()
            ),
            Self::ShorterThanHeader { len } => {
                write!(
                    f,
                    "the object is shorter than a Container header ({len} bytes)"
                )
            }
            Self::ShorterThanMetaSection { len, declared } => write!(
                f,
                "the object is shorter than the meta section its header declares ({len} bytes, \
                 {declared} declared)"
            ),
            Self::UnnamableKey { found } => {
                write!(
                    f,
                    "a payload map key is {found:?}, which this dump cannot name"
                )
            }
            Self::UnrenderableValue { found } => {
                write!(
                    f,
                    "a payload carries {found:?}, which this dump cannot render"
                )
            }
            Self::InSet { producer, .. } => write!(f, "in the fixture set {producer:?} wrote"),
            Self::InFixture { kind, fixture, .. } => write!(f, "{kind} fixture {fixture:?}"),
            Self::InEntry { index, .. } => write!(f, "entry {index}"),
        }
    }
}
