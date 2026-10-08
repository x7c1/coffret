//! The fifteen things a browser may ask of a Library.
//!
//! Three of them are about what the Library holds and answer out of the
//! catalog alone; the fourth is the only one that reaches Storage for bytes,
//! and it reaches it for one Entry at a time. There is deliberately no route
//! that lists Storage, and none that hands anything encrypted out: what crosses
//! this boundary is plaintext the device already has, or is about to place, and
//! nothing else.
//!
//! The refresh is the fifth, and the one that reaches Storage for no bytes at
//! all: it replays what the Journal holds into the catalog (spec: CK-9) and says
//! what changed. It is how a device that has just joined, or one another device
//! has committed past, learns there is anything to show — and it hands over
//! counts rather than content.
//!
//! One goes the other way. The upload takes files somebody dropped into the
//! folder this device maps and arms the flow that carries them in — a sync for
//! an ordinary drop, which is the same gesture as copying them in and typing
//! `coffret sync`, and a freeze where the drop is a book being brought into a
//! folder made for it (spec: PK-17). It is where a Library gains anything
//! through the explorer, and it gains it through the flows the command line uses
//! rather than through one of its own.
//!
//! The next four are about the work that runs in the background — the fill
//! that brings over the rest of the folder somebody opened a file in, the sync
//! that carries in what they dropped, and the freeze that packs a book they
//! brought in or a folder they asked to have packed. One says how far all
//! three have got; the other three arm one — taking it up again after it was
//! left unfinished, or, for the freeze, packing a folder somebody chose — which
//! arms Storage work rather than doing any of it while the request is open.
//! None of the four is another way to ask for bytes.
//!
//! The freeze is also read before it is armed: a `GET` of it counts what a
//! freeze of the folder would pack, and what it would leave out, by the
//! freeze's own scan stopped before a file is read. It changes nothing, and
//! it is how the explorer shows somebody what "Pack this folder…" will do
//! before they say yes.
//!
//! And one is about how this device reaches Storage at all: the reconnect,
//! which renews a grant Storage has stopped taking by running the consent flow
//! inside the server and handing the page the consent page to open. It needs
//! the Library's keys and asks for no Passphrase, since the open Library
//! already holds what the renewal writes under.
//!
//! And two are about this device rather than the Library: the browse, which
//! lists the folders on this device's own disk, and the map, which records that
//! one of them holds the Library root or a top-level folder of it (spec: EP-9).
//! A page cannot be handed a real path on the device, so this is how the
//! explorer chooses one. Neither needs a key — a mapping is the device's record
//! (spec: CK-7) — and both are still refused while the Library is locked,
//! because the gesture they serve is only ever offered over a listing.
//!
//! None of them locks the Library. It locks after the idle interval (spec:
//! DK-4), and stopping the server ends its hold on the keys as well; a route
//! that did the same thing would be a third way to arrive at a state two ways
//! already reach.
//!
//! One is the way back: the unlock, which asks the process this server runs in
//! to take the Passphrase again in a window of its own (spec: DK-1). It carries
//! no Passphrase — none crosses this boundary in either direction — and where
//! there is no such window, as under the command line, it answers with the
//! locked refusal that says to start the server again.
//!
//! Three of the fifteen go on answering once the Library is locked — which
//! Library this is, this server's account of what it was doing, and the unlock —
//! because none of them needs a key and a locked server is still one a person
//! should be able to read the name of, and ask to have opened. Every other one
//! meets a locked server with the same refusal, which says the Passphrase is
//! required and where it is entered (spec: DK-2): the browse and the map among
//! them, for the reason given above.
//!
//! One of those three answers more than its own subject: the account of what
//! this server was doing carries which of the two states this device holds the
//! Library in, so a window left open over a page it decrypted hears of a lock
//! nobody asked it about (spec: DK-4) rather than waiting for its next request
//! to be refused.
//!
//! Those that name a place in the Library take it as `?path=`, for the reason
//! [`PathQuery`](crate::entry_query::PathQuery) gives.
//!
//! Beside the fifteen are two answers that are not routes at all: one for a path
//! none of them is registered at, and one for a path of theirs asked by a
//! method it does not take. They are here so that nothing this server answers
//! leaves the one shape a refusal takes.

mod browse;
pub use browse::browse;

mod file;
pub use file::file;

mod fill;
pub use fill::fill;

mod folders;
pub use folders::folders;

mod freeze;
pub use freeze::{freeze, preview as preview_freeze};

mod library;
pub use library::library;

mod list;
pub use list::list;

mod map;
pub use map::map;

mod nowhere;
pub use nowhere::{no_such_method, no_such_route};

mod reconnect;
pub use reconnect::reconnect;

mod refresh;
pub use refresh::refresh;

mod refusal_dto;
use refusal_dto::RefusalDto;

mod sync;
pub use sync::sync;

mod unlock;
pub use unlock::unlock;

mod upload;
pub use upload::upload;

mod work;
pub use work::work;
