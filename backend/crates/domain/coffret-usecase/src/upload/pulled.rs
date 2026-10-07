use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, ReadBuf};
use tokio::time::{self, Instant, MissedTickBehavior};

/// How often, at most, an upload in flight says how far it has got.
///
/// Often enough that a line rewritten from it visibly moves, and seldom enough
/// that a browser polling the work answer, or a terminal redrawing its line, is
/// not handed a report per frame of a body Storage pulls in kilobytes.
pub(crate) const REPORT_EVERY: Duration = Duration::from_millis(250);

/// How many bytes a store has pulled from the body of the put under way.
///
/// The port hands a store a stream (see [`ByteStream`](crate::ByteStream)) and
/// learns nothing more until the put answers, so the one place this layer can
/// see a transfer move is the reader it hands over. The count sits behind an
/// [`Arc`] because the reader has to be `'static` to travel inside the stream,
/// while what reads the count is the flow that is waiting on the put.
#[derive(Clone, Default)]
pub(super) struct Pulled(Arc<AtomicU64>);

impl Pulled {
    /// Wraps the reader one attempt of a put sends, starting the count over.
    ///
    /// Over, because an attempt that is tried again sends its object from the
    /// first byte, and the count is of what is under way.
    pub(super) fn counting<R>(&self, reader: R) -> Counting<R> {
        self.0.store(0, Ordering::Relaxed);
        Counting {
            inner: reader,
            pulled: Arc::clone(&self.0),
        }
    }

    /// How many bytes the attempt under way has pulled so far.
    pub(super) fn get(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

/// A reader that adds what is read through it to a [`Pulled`].
pub(super) struct Counting<R> {
    inner: R,
    pulled: Arc<AtomicU64>,
}

impl<R: AsyncRead + Unpin> AsyncRead for Counting<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buffer.filled().len();
        let polled = Pin::new(&mut self.inner).poll_read(context, buffer);
        if let Poll::Ready(Ok(())) = polled {
            let read = buffer.filled().len() - before;
            self.pulled.fetch_add(read as u64, Ordering::Relaxed);
        }
        polled
    }
}

/// Runs `work`, calling `report` every [`REPORT_EVERY`] until it is done.
///
/// The first call is one period in rather than at once, because whoever is
/// waiting on `work` has just been told where it starts. What `report` says is
/// its own business — it is handed nothing — and nothing is called once `work`
/// has answered, so the report at the end is the caller's, made with the answer
/// in hand.
pub(super) async fn reporting<T>(work: impl Future<Output = T>, mut report: impl FnMut()) -> T {
    tokio::pin!(work);
    let mut ticks = time::interval_at(Instant::now() + REPORT_EVERY, REPORT_EVERY);
    // A tick missed while the runtime was busy is a report that would say what
    // the next one says; there is no backlog of them worth catching up on.
    ticks.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            // The answer first: a put that has finished is past anything a
            // report of it part way could say.
            biased;
            answer = &mut work => return answer,
            _ = ticks.tick() => report(),
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncReadExt;

    use super::*;

    #[tokio::test]
    async fn what_is_read_through_it_is_counted() {
        let pulled = Pulled::default();
        let mut reader = pulled.counting(std::io::Cursor::new(vec![7u8; 100]));

        let mut front = [0u8; 40];
        reader.read_exact(&mut front).await.expect("a cursor reads");
        assert_eq!(pulled.get(), 40);

        let mut rest = Vec::new();
        reader.read_to_end(&mut rest).await.expect("a cursor reads");
        assert_eq!(pulled.get(), 100);
    }

    #[tokio::test]
    async fn another_attempt_counts_from_nothing() {
        let pulled = Pulled::default();
        let mut first = pulled.counting(std::io::Cursor::new(vec![7u8; 100]));
        first
            .read_to_end(&mut Vec::new())
            .await
            .expect("a cursor reads");

        let _second = pulled.counting(std::io::Cursor::new(vec![7u8; 100]));
        assert_eq!(pulled.get(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn a_long_wait_is_reported_on_and_a_short_one_is_not() {
        let mut reports = 0;
        reporting(time::sleep(REPORT_EVERY * 4 + REPORT_EVERY / 2), || {
            reports += 1
        })
        .await;
        assert_eq!(reports, 4, "one report per period, none at the start");

        let mut reports = 0;
        reporting(async {}, || reports += 1).await;
        assert_eq!(reports, 0, "what answers at once has nothing to report");
    }
}
