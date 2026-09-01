use std::time::Duration;

use tokio::time::timeout;
use tracing::error;

use crate::state::ServerState;

use super::run;

/// How long the catch-up may keep the socket unbound.
///
/// There has to be one, because nothing below this adds up to a bound on
/// *starting up*. Each Storage call is bounded — the Drive transport gives up on
/// a connection that takes more than twenty seconds and on a transfer that goes
/// a minute without a byte, and the retry policy gives up after six attempts and
/// two minutes of waiting — but a catch-up is one call per Journal record this
/// device has not seen, so what those bound is every step and not the walk. A
/// device joining a Library with a long history, over a network that answers
/// slowly rather than not at all, would leave the port unbound for as long as
/// the whole walk took; and a provider that dribbles a byte a minute is inside
/// every one of those bounds forever.
///
/// A minute is chosen against the same constants. It is well past the twenty
/// seconds the transport spends on a connection nothing is listening at, so the
/// ordinary "Storage is unreachable" case is still reported in Storage's own
/// words rather than swallowed by this; and it is under the two minutes the
/// retry policy may spend sleeping, so a single throttled call cannot spend the
/// whole of startup on backoff alone. Above it, what is being waited for is no
/// longer worth an unbound port: the Index is on disk, the explorer over it
/// works offline, and the refresh control asks again.
const DEADLINE: Duration = Duration::from_secs(60);

/// Catches it up once as the process starts, and serves either way.
///
/// A device that has just joined holds nothing, and one whose Library another
/// device has written to since holds what it held then; both would open an
/// explorer showing a Library that is not the Library. So the server asks before
/// it answers anything.
///
/// A failure here is not fatal, and that is the design rather than leniency:
/// reading what the Index already holds needs no Storage at all, and a server
/// that refused to start because a bucket was unreachable would take the offline
/// half of the explorer down with the online half. It is recorded the way every
/// refusal the background work meets is recorded — the whole chain into the log,
/// nothing invented for a screen — and the refresh control is what retries it.
///
/// A Storage that never answers is the same verdict arrived at differently, and
/// so is bounded rather than waited on: a filtered network takes the connection
/// and says nothing, and without the `DEADLINE` this module keeps that silence
/// would be a server that never binds — which is the one failure this whole
/// arrangement is meant to rule out, and the loudest form of it, since a refusal
/// at least ends.
///
/// # Giving up mid-replay does not damage the catalog
///
/// The deadline drops the catch-up wherever it had got to, and that is safe by
/// construction. Records are replayed one at a time, and each one is a single
/// Index transaction that either commits whole or not at all — a dropped future
/// cannot tear one, because the write runs on a blocking task the drop does not
/// reach. So the catalog is left standing at the last record that committed, and
/// its checkpoint says so. That is exactly the state the next catch-up reads:
/// the replay walks up from the checkpoint and steps over what it already
/// covers, which is the convergence two replayers over one catalog already rely
/// on (spec: CK-9). Nothing is lost but the wait — a refresh, or the next start,
/// carries on from the checkpoint rather than from the beginning.
///
/// It is deliberately not put on the activity route beside the fill and the
/// sync. Those are followed because something is *running*; this is over before
/// the socket is bound, and an explorer with nothing in flight asks for the
/// activity exactly never — so a browser would be the last thing to hear of it.
/// What a person does instead is press refresh, which meets the same Storage and
/// says so in the same words.
pub async fn catch_up_at_startup(state: &ServerState) {
    match timeout(DEADLINE, run::catch_up(state, "startup")).await {
        Ok(Ok(_outcome)) => {}
        Ok(Err(refusal)) => refusal.record("startup"),
        Err(_elapsed) => gave_up(),
    }
}

/// Records a catch-up abandoned at the deadline, the way a refused one is
/// recorded.
///
/// The same level and the same `operation`, because to whoever reads the log
/// these are one event — the catalog is not at the Library's head, and the
/// server is about to serve anyway. What is said instead of a cause is the
/// deadline itself: there is no error under this, only a call that had not come
/// back, and naming the bound is what tells a reader whether to look at their
/// network or at this constant.
fn gave_up() {
    error!(
        operation = "startup",
        deadline_ms = DEADLINE.as_millis(),
        "Storage did not answer within the startup deadline; serving what the Index holds",
    );
}
