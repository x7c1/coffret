use std::path::Path;

use async_trait::async_trait;

use crate::descent_error::DescentError;
use crate::destination::Destination;
use crate::device_state::RootMarkerId;
use crate::standing::Standing;

/// Everything the flows ask of the places this device writes a fetched Entry
/// into.
///
/// The third of the capabilities over this device's own disk, beside the
/// [`Spool`](crate::Spool) a Container waits in and the
/// [`MappedRoots`](crate::MappedRoots) a scan reads. Nothing here crosses the
/// trust boundary the [`ObjectStore`](crate::ObjectStore) port exists to cross;
/// it is a capability for the reason the other two are, and the most sharply of
/// the three. What EP-11 promises about a placement is a promise about
/// *interruption* — a write that fails, a flush that fails, a rename that fails,
/// a cleanup that fails, an Index update that fails after the file is already
/// visible — and a real filesystem cannot be asked to refuse a chosen one of
/// those steps, which leaves every one of the recovery rules untested.
///
/// What is deliberately *not* here is every decision about where a file belongs
/// and whether it may be written. Which Entry goes where is the mappings'
/// (spec: EP-9), whether the device may claim the path is the selection's
/// (spec: EP-10, EP-11), the scratch-name convention is
/// [`scratch`](crate::scratch)'s, and the order — write, flush, hold the
/// plaintext against the catalog, stamp, publish, mark present — is the fetch's.
/// What moves behind this line is only how the operating system is asked: how a
/// folder is reached without following a link, how a file is made exclusively,
/// how bytes get to the device, and how a name is stamped and renamed against an
/// open folder.
///
/// Both operations take the mapped root and the Entry Path's components below it
/// *apart*, and that is the whole point of the signatures. An Entry Path comes
/// from another enrolled device and says nothing about the shape of this
/// device's disk: the same `link/authorized_keys` is an ordinary folder and a
/// file over there and may be a symbolic link out of the mapped root here. A
/// capability handed a joined path could only ask the operating system to
/// resolve it, following that link and writing where the Library never pointed.
/// Handed the components, it walks them one at a time and refuses anything that
/// is not a real folder of that root (spec: EP-4, EP-11).
///
/// The writing operation takes one thing more: the identity the mapping records
/// for its root, which [`reach`](Self::reach) holds the root's marker against
/// from the handle the placement then writes through (spec: EP-13). Reading asks
/// nothing of the kind: [`look_up`](Self::look_up) places nothing, and a root
/// whose identity is wrong is a root a *write* may not touch.
///
/// Every operation fails with [`DescentError`], and the contract's other half is
/// not in the signatures: **a path this device cannot materialize is
/// [`Blocked`](DescentError::Blocked) and never an I/O refusal**. Reading the
/// errno that says so is the gateway's, exactly as swallowing the absence a
/// [`discard`](crate::Spool::discard) tolerates is (spec: OC-6).
///
/// The trait is object safe, so a flow holds `&dyn Destinations` and is written
/// once against the device's own folders and against the in-memory fake alike.
#[async_trait]
pub trait Destinations: Send + Sync {
    /// Opens the folder one file belongs in, once the root has proved to be the
    /// root the mapping was recorded against, making the folders that are not
    /// there yet.
    ///
    /// `components` is the Entry Path's components below the mapping's prefix,
    /// its last being the file's own name — which is kept on the
    /// [`Destination`] rather than reached, because every write is then made
    /// against the open folder by name.
    ///
    /// `expected` is the identity that mapping recorded for its root
    /// ([`Mapping::expected_root_id`](crate::device_state::Mapping::expected_root_id)),
    /// and it is checked here rather than anywhere else because this is the one
    /// call every placement goes through. The root is opened as the person named
    /// it — which may pass through a symbolic link, that being their
    /// configuration to make (spec: EP-8, EP-9) — and then, *from that open
    /// handle*, `.coffret` and `root` are descended without following links, the
    /// marker is read and parsed, and its identity is held against `expected`.
    /// The components are descended from the same handle afterwards, so no
    /// placement is ever made against a root that was resolved a second time:
    /// re-resolving it would leave exactly the race the rule rules out
    /// (spec: EP-13). `None` is not a mapping that skips the check — there is
    /// nothing for the marker to agree with, so nothing may be placed.
    ///
    /// Nothing about the marker is created or repaired here, whatever is found:
    /// only recording a mapping writes or adopts one (spec: EP-13). The mapped
    /// root itself is not made either, for the same reason — a root that is not
    /// there carries no marker, so a fetch into one refuses instead of making a
    /// folder nobody registered.
    ///
    /// The folders *below* the root are made, because an Entry Path's separators
    /// are the whole of what a folder is (spec: EP-2): a device fetching
    /// `albums/2026/spring.jpg` into an empty mapped root has to make both.
    ///
    /// # Errors
    ///
    /// [`DescentError::Refused`], carrying the root and which of EP-13's cases
    /// it was, where the root is not the one the mapping expects. Nothing below
    /// the root is touched in that case, not even a folder that would have been
    /// made.
    ///
    /// [`DescentError::Blocked`], naming the component it stopped at, where
    /// something on the way down is not a real folder of that root — a symbolic
    /// link, or an ordinary file where a folder must be. The Entry Path cannot be
    /// materialized on this device, whatever the link points at (spec: EP-4,
    /// EP-11). The mapped root itself is among the names that can fail that way,
    /// since nothing is made here: a path the person configured that turns out to
    /// be a file is not a folder the Library's subtree can stand in.
    /// [`DescentError::Io`] where the operating system refused for any other
    /// reason, a mapped root that is not there among them.
    async fn reach(
        &self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        components: &[String],
    ) -> Result<Box<dyn Destination>, DescentError>;

    /// What stands at the file's own name, making nothing on the way.
    ///
    /// The same walk as [`reach`](Self::reach) over the folders that are already
    /// there. `Ok(None)` is an empty place, and it stands for a folder on the way
    /// being absent as much as for the file itself being absent: nothing can
    /// stand at the file's path if the folder above it does not exist, however
    /// few of them have been made.
    ///
    /// What it finds is stated *without* following links, for the reason a scan
    /// stats a directory entry that way: a symbolic link is not the file it
    /// points at (spec: EP-8), so one standing at the file's own name comes back
    /// as a [`Standing`] that is not a file rather than as an empty place.
    ///
    /// # Errors
    ///
    /// [`DescentError::Blocked`] where a component on the way down is a symbolic
    /// link or is not a folder — at any depth, and whether the link points inside
    /// the mapped root or out of it, because the canonical place for the Entry is
    /// the one the mappings name and a second name for it is not that place
    /// (spec: EP-9, EP-4). [`DescentError::Io`] where the operating system
    /// refused for any other reason.
    async fn look_up(
        &self,
        root: &Path,
        components: &[String],
    ) -> Result<Option<Standing>, DescentError>;
}
