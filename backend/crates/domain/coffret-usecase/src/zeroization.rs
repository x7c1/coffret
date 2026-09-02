//! What the secret-bearing inventory promises, asserted against every type on
//! it.
//!
//! The inventory itself is in [`coffret_model::MasterKey`]'s module, beside the
//! type the key hierarchy hangs from. The assertions are here because this is
//! the one crate that can see all three layers the list spans — the domain
//! types, the byte forms they take, and the bundles the flows work under — so
//! one file covers the whole list rather than three files each covering a
//! third of it.
//!
//! Both promises are pinned:
//!
//! - `zeroizes_on_drop` is a compile-time check. It takes the types by a bound,
//!   so a type that stops wiping itself — a `Drop` deleted, a marker no longer
//!   implemented — fails to build here.
//! - `is_not_clone` is a run-time check over a compile-time fact, since Rust
//!   offers no way to require the *absence* of a trait in a bound. An inherent
//!   associated function shadows a blanket trait one, so the probe answers
//!   `true` for exactly the types that implement `Clone` — and a `#[derive]`
//!   added to any of these fails the case by name.
//!
//! What the pair does not check is the wipe itself: reading a buffer back after
//! its owner is gone takes the `unsafe` the domain crates forbid, and DK-7 says
//! as much — it is an absence claim over the whole process image, honored by
//! construction. Each type's own tests cover the operation its `Drop` runs.

use std::marker::PhantomData;

use coffret_format::{PurposeKey, RecoveryCode, UnlockedMasterKey};
use coffret_model::{ContainerKey, MasterKey, Passphrase};
use zeroize::ZeroizeOnDrop;

use crate::commit::ControlKeys;
use crate::library_keys::LibraryKeys;

/// Accepts only a type that wipes its bytes when it is dropped.
const fn zeroizes_on_drop<T: ZeroizeOnDrop>() {}

/// The probe `is_clone` is answered through.
struct Probe<T>(PhantomData<T>);

/// The answer for a type that is not `Clone`, as a blanket trait implementation.
trait NotClone {
    fn is_clone() -> bool {
        false
    }
}

impl<T> NotClone for Probe<T> {}

/// The answer for a type that is, as an inherent one — which method resolution
/// reaches before the trait above.
impl<T: Clone> Probe<T> {
    fn is_clone() -> bool {
        true
    }
}

// DK-7: every type on the inventory wipes itself when it is dropped. This does
// not run — building it is the assertion.
#[test]
fn every_secret_bearing_type_zeroizes_on_drop() {
    zeroizes_on_drop::<Passphrase>();
    zeroizes_on_drop::<MasterKey>();
    zeroizes_on_drop::<ContainerKey>();
    zeroizes_on_drop::<UnlockedMasterKey>();
    zeroizes_on_drop::<RecoveryCode>();
    zeroizes_on_drop::<PurposeKey>();
    zeroizes_on_drop::<ControlKeys>();
    zeroizes_on_drop::<LibraryKeys>();
}

// DK-7: none of them is `Clone`, so no copy of key material is made by a
// `#[derive]` nobody reads. A caller that needs one value in two places borrows
// it, moves it, or shares it through an `Arc`.
#[test]
fn no_secret_bearing_type_is_clone() {
    assert!(!Probe::<Passphrase>::is_clone(), "Passphrase");
    assert!(!Probe::<MasterKey>::is_clone(), "MasterKey");
    assert!(!Probe::<ContainerKey>::is_clone(), "ContainerKey");
    assert!(!Probe::<UnlockedMasterKey>::is_clone(), "UnlockedMasterKey");
    assert!(!Probe::<RecoveryCode>::is_clone(), "RecoveryCode");
    assert!(!Probe::<PurposeKey>::is_clone(), "PurposeKey");
    assert!(!Probe::<ControlKeys>::is_clone(), "ControlKeys");
    assert!(!Probe::<LibraryKeys>::is_clone(), "LibraryKeys");
}

// The probe is only worth anything if it can tell the two apart, so it is
// checked against a type that is `Clone` on purpose.
#[test]
fn the_probe_answers_for_a_type_that_is_clone() {
    assert!(Probe::<String>::is_clone());
}
