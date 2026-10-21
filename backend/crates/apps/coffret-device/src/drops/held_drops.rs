use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use coffret_model::lowercase_hex;
use coffret_usecase::drop_walk::DropWalk;
use tokio::time::Instant;
use tracing::{info, warn};

use super::{DropRefusal, Dropped, DroppedRow, DroppedSet, NotTakenRow};

/// How long a drop is held for the import that carries it in: ten minutes,
/// which is longer than anybody looks at the confirmation a drop is answered
/// with and short enough that a handle nobody used is not a standing way to
/// the files it names.
pub const DROP_LIFETIME: Duration = Duration::from_secs(10 * 60);

/// How many drops are held at once. A fifth ends the oldest: a drop is one
/// gesture, and four still waiting for their import is already more than one
/// window makes.
pub const HELD_DROPS: usize = 4;

/// How many bytes of the operating system's CSPRNG a handle is drawn from.
const HANDLE_BYTES: usize = 32;

/// The drops this process holds, each under its one-time handle.
///
/// A handle is drawn from the operating system's CSPRNG and is the only way to
/// the files it names: the import names it, takes the set out, and the handle
/// is gone. One that is not taken ends after [`DROP_LIFETIME`], and holding a
/// drop past [`HELD_DROPS`] ends the oldest. A handle is never put in an event
/// (spec: EL-1).
#[derive(Default)]
pub struct Drops {
    held: Mutex<VecDeque<Held>>,
}

/// One drop, held.
struct Held {
    handle: String,
    set: DroppedSet,
    at: Instant,
}

impl Drops {
    /// Nothing held.
    pub fn new() -> Self {
        Self::default()
    }

    /// Holds what `walk` took under a fresh handle, and says what the page is
    /// told of it.
    ///
    /// A walk that took nothing holds nothing, and is answered with no handle.
    /// A handle that cannot be drawn refuses the drop: one anything could
    /// guess would be no handle at all.
    pub fn hold(&self, walk: DropWalk) -> Result<Dropped, getrandom::Error> {
        let files = walk
            .taken
            .iter()
            .map(|source| DroppedRow {
                path: source.path.clone(),
                size: source.size,
                mtime_millis: source.mtime.as_unix_seconds().saturating_mul(1000),
            })
            .collect();
        let not_taken: Vec<NotTakenRow> =
            walk.not_taken.into_iter().map(NotTakenRow::from).collect();
        if walk.taken.is_empty() {
            return Ok(Dropped {
                handle: None,
                files,
                not_taken,
            });
        }
        let mut bytes = [0u8; HANDLE_BYTES];
        getrandom::fill(&mut bytes)?;
        let handle = lowercase_hex::encode(&bytes);

        let now = Instant::now();
        let mut held = self.held();
        held.retain(|one| !expired(one, now));
        while held.len() >= HELD_DROPS {
            held.pop_front();
            info!(
                operation = "hold_drop",
                "a drop nobody imported was ended by a later one",
            );
        }
        held.push_back(Held {
            handle: handle.clone(),
            set: DroppedSet::new(walk.taken),
            at: now,
        });
        Ok(Dropped {
            handle: Some(handle),
            files,
            not_taken,
        })
    }

    /// Takes the drop held under `handle`, which is then held no longer.
    pub fn take(&self, handle: &str) -> Result<DroppedSet, DropRefusal> {
        let now = Instant::now();
        let mut held = self.held();
        let Some(position) = held.iter().position(|one| one.handle == handle) else {
            warn!(
                operation = "take_drop",
                refusal = "unknown",
                "a drop handle took nothing"
            );
            return Err(DropRefusal::Unknown);
        };
        let one = held
            .remove(position)
            .expect("the position was found a moment ago under the same lock");
        if expired(&one, now) {
            warn!(
                operation = "take_drop",
                refusal = "expired",
                "a drop handle took nothing"
            );
            return Err(DropRefusal::Expired);
        }
        info!(
            operation = "take_drop",
            files = one.set.len(),
            "a held drop was taken for its import",
        );
        Ok(one.set)
    }

    /// The queue, whatever a thread that panicked while holding it left in it.
    ///
    /// Every write is one insertion or one removal, so there is no half-made
    /// state for a panic to have left behind.
    fn held(&self) -> MutexGuard<'_, VecDeque<Held>> {
        self.held.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Whether a held drop has outlived [`DROP_LIFETIME`] by `now`.
fn expired(held: &Held, now: Instant) -> bool {
    now.saturating_duration_since(held.at) >= DROP_LIFETIME
}
