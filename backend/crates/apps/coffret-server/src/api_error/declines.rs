//! A fetch's verdict on one Entry it did not place (spec: EP-11).

use coffret_device::{FetchError, Redacted, Surfaced};

use super::ApiError;

impl ApiError {
    /// A fetch declined the path, and said why (spec: EP-11).
    ///
    /// A key-lost Container is its own reason rather than one finding among the
    /// others, because it is the one of them nothing about this device
    /// remedies: the ciphertext is where it belongs and the key is gone
    /// (spec: KL-7, KL-17).
    pub fn declined(surfaced: &Surfaced) -> Self {
        let (reason, message) = match surfaced {
            Surfaced::KeyLost { .. } => (
                "key_lost",
                "the Library records no key for the Container holding this Entry",
            ),
            Surfaced::ForeignFile { .. } => (
                "surfaced",
                "a file this device did not put there stands where this Entry belongs",
            ),
            Surfaced::LocallyChanged { .. } => (
                "surfaced",
                "what this device wrote there has since changed or gone",
            ),
            Surfaced::WitnessedDeletion { .. } => (
                "surfaced",
                "this device witnessed the deletion of this Entry's file",
            ),
            // The folder the descent stopped at stays out of the sentence, the
            // way every other local path does on these routes: it is named to
            // whoever is at a terminal keeping the Library, and this is one line
            // beside one row in a browser.
            Surfaced::UnreachablePlace { .. } => (
                "surfaced",
                "a folder on the way to this Entry is not a folder of this device's mapped \
                 folder",
            ),
            // The name is in the sentence because it is a name the person never
            // chose: a path carrying it came from whichever device committed it,
            // and recognizing the component is the whole of reading the line.
            //
            // *Or a spelling of it*, because a selection refuses a component
            // that only folds to the name as well (spec: EP-14) — the one place
            // the two are one finding, a placement refusing both alike — and the
            // component this is about may therefore be `.COFFRET`, which a
            // sentence offering `.coffret` alone would have somebody hunting
            // for in a path that does not hold it.
            Surfaced::ReservedComponent { .. } => (
                "surfaced",
                "this Entry's path carries `.coffret`, or a name differing from it only in \
                 case: that is coffret's own folder inside a mapped folder and never a place a \
                 file is put",
            ),
        };
        Self::declined_because(reason, Some(name_of(surfaced)), message)
    }

    pub(super) fn declined_as(reason: &'static str, message: &str, cause: FetchError) -> Self {
        Self::declined_because(reason, None, message).caused_by(cause.redacted())
    }
}

/// The name the device layer gives one finding (spec: EP-11).
pub(super) fn name_of(surfaced: &Surfaced) -> &'static str {
    match surfaced {
        Surfaced::ForeignFile { .. } => "ForeignFile",
        Surfaced::LocallyChanged { .. } => "LocallyChanged",
        Surfaced::WitnessedDeletion { .. } => "WitnessedDeletion",
        Surfaced::UnreachablePlace { .. } => "UnreachablePlace",
        Surfaced::KeyLost { .. } => "KeyLost",
        Surfaced::ReservedComponent { .. } => "ReservedComponent",
    }
}
