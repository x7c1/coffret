use std::sync::Once;
use std::time::Duration;

use tracing::warn;

/// A duration drawn uniformly from zero to `interval`.
pub(super) fn full_jitter(interval: Duration) -> Duration {
    let Some(drawn) = draw() else {
        // The unjittered interval is the wait this was spreading out, so a call
        // that drew nothing still backs off — it merely backs off in step with
        // anything else that drew nothing.
        return interval;
    };
    // Nanoseconds, because a ceiling of a few hundred milliseconds quantized to
    // whole seconds would be a choice between no wait and the whole interval.
    let span = u64::try_from(interval.as_nanos())
        .unwrap_or(u64::MAX)
        .saturating_add(1);

    Duration::from_nanos(drawn % span)
}

/// Guards the one event a process emits about entropy it was refused.
static ENTROPY_REFUSED: Once = Once::new();

/// Eight bytes from the operating system's CSPRNG, or nothing.
fn draw() -> Option<u64> {
    let mut bytes = [0u8; 8];
    if let Err(error) = getrandom::fill(&mut bytes) {
        // Not a failure of the call — a jitter is not a nonce, and where coffret
        // does need one it refuses outright instead. But a wait that silently
        // stopped being jittered is exactly what explains a thundering herd
        // months later, so it is recorded at a level the default sink keeps.
        //
        // Once per process rather than once per wait: an operating system that
        // refuses entropy refuses it every time, and a line per wait would
        // spend the log's byte ceiling evicting the evidence it is kept for.
        ENTROPY_REFUSED.call_once(|| {
            warn!(detail = %error, "backing off without jitter: no entropy to draw it from");
        });
        return None;
    }
    Some(u64::from_ne_bytes(bytes))
}
