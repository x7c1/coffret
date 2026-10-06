//! The eleven things a browser may ask of a Library.
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
//! The last four are about the work nobody asked for — the fill that brings over
//! the rest of the folder somebody opened a file in, the sync that carries in
//! what they dropped, and the freeze that packs a book they brought in. One says
//! how far all three have got; the other three take one up again after it was
//! left unfinished — Storage stopped it, or somebody clicking elsewhere took the
//! fill away — which arms Storage work rather than doing any of it while the
//! request is open. None of the four is another way to ask for bytes.
//!
//! And one is about how this device reaches Storage at all: the reconnect,
//! which renews a grant Storage has stopped taking by running the consent flow
//! inside the server and handing the page the consent page to open. It needs
//! the Library's keys and asks for no Passphrase, since the open Library
//! already holds what the renewal writes under.
//!
//! None of them locks the Library. It locks after the idle interval (spec:
//! DK-4), and stopping the server ends its hold on the keys as well; a route
//! that did the same thing would be a third way to arrive at a state two ways
//! already reach.
//!
//! Two of the eleven need no key at all — which Library this is, and this
//! server's account of what it was doing — and those two are exactly the ones
//! that go on answering once the Library is locked. Every other one meets a
//! locked server with the same refusal, which says the Passphrase is required
//! (spec: DK-2).
//!
//! One of those two answers more than its own subject: the account of what
//! this server was doing carries which of the two states this device holds the
//! Library in, so a window left open over a page it decrypted hears of a lock
//! nobody asked it about (spec: DK-4) rather than waiting for its next request
//! to be refused.
//!
//! Those that name a place in the Library take it as `?path=`, for the reason
//! [`PathQuery`](crate::entry_query::PathQuery) gives.
//!
//! Beside the eleven are two answers that are not routes at all: one for a path
//! none of them is registered at, and one for a path of theirs asked by a
//! method it does not take. They are here so that nothing this server answers
//! leaves the one shape a refusal takes.

mod file;
pub use file::file;

mod fill;
pub use fill::fill;

mod folders;
pub use folders::folders;

mod freeze;
pub use freeze::freeze;

mod library;
pub use library::library;

mod list;
pub use list::list;

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

mod upload;
pub use upload::upload;

mod work;
pub use work::work;
