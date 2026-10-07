use serde::Serialize;

use coffret_device::{ByteCount, Phase, Step};

/// How far into a run the flow reports it has got.
///
/// The [`Step`](coffret_device::Step) the use case layer reports, in the shape a
/// browser reads it in. It is the same value the command line renders its
/// progress line from, and deliberately so: one run has one answer about where
/// it is, whichever shell is watching.
#[derive(Serialize)]
pub(super) struct StepDto {
    /// Which phase of the flow it is in.
    phase: &'static str,
    /// How many units of that phase are finished.
    done: usize,
    /// How many there are in all, and `null` where the phase cannot say.
    ///
    /// Absent rather than zero, and the two must not be shown alike: a phase
    /// that cannot count its work — a catch-up learns what it has to replay by
    /// replaying it — is exactly the phase that goes quiet for minutes, and a
    /// `0` there would read as a phase with nothing in it.
    total: Option<usize>,
    /// How many bytes of the phase Storage has taken, out of how many it sends,
    /// and `null` for a phase that does not count them.
    bytes: Option<ByteCountDto>,
}

/// How many bytes of a phase have gone, out of how many it sends.
#[derive(Serialize)]
struct ByteCountDto {
    done: u64,
    total: u64,
}

impl StepDto {
    pub(super) fn of(step: &Step) -> Self {
        Self {
            phase: named(step.phase),
            done: step.done,
            total: step.total,
            bytes: step
                .bytes
                .map(|ByteCount { done, total }| ByteCountDto { done, total }),
        }
    }
}

/// The word one phase travels under.
///
/// Named here rather than on [`Phase`](coffret_device::Phase) because it is this
/// route's vocabulary: the use case layer says what a run is doing, and what a
/// browser calls it is the browser's business. Matched exhaustively, so a phase
/// added to the flow stops this compiling until somebody says what a person
/// reading a status bar should be told it is.
fn named(phase: Phase) -> &'static str {
    match phase {
        Phase::CatchingUp => "catching_up",
        Phase::Settling => "settling",
        Phase::Scanning => "scanning",
        Phase::Packing => "packing",
        Phase::Uploading => "uploading",
        Phase::Fetching => "fetching",
    }
}

#[cfg(test)]
mod tests {
    use coffret_device::{ByteCount, Phase, Step};
    use serde_json::json;

    use super::StepDto;

    // What the explorer's packing line reads its megabytes from, beside the
    // count it already had.
    #[test]
    fn an_upload_step_carries_its_bytes() {
        let step = Step::new(Phase::Uploading, 0, 1).with_bytes(ByteCount {
            done: 23_000_000,
            total: 60_000_000,
        });
        assert_eq!(
            serde_json::to_value(StepDto::of(&step)).expect("a step serializes"),
            json!({
                "phase": "uploading",
                "done": 0,
                "total": 1,
                "bytes": { "done": 23_000_000, "total": 60_000_000 },
            }),
        );
    }

    // Said as `null` rather than left out, so a client reads one shape for
    // every step and tells "not counted" from a field it does not know.
    #[test]
    fn a_step_that_counts_no_bytes_says_so() {
        assert_eq!(
            serde_json::to_value(StepDto::of(&Step::new(Phase::Packing, 2, 5)))
                .expect("a step serializes"),
            json!({ "phase": "packing", "done": 2, "total": 5, "bytes": null }),
        );
    }
}
