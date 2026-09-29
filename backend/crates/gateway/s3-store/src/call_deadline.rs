//! How long a call to S3 may take.
//!
//! Three kinds of call, each bounded by what it carries. A call whose body is
//! small both ways — a listing, a `HEAD`, a delete, the probes a Library's first
//! moments make — and that has not finished within [`SMALL_CALL_DEADLINE`] is
//! not going to: it is ended there, and arrives as [`Error::Timeout`], which the
//! retry policy already retries. A call that sends a body — the upload in
//! [`ObjectStore::put`] and the conditional create in
//! [`ObjectStore::put_if_absent`] — is given that deadline plus the time its
//! body takes at [`SLOWEST_UPLOAD_RATE`] (`for_body_of`), which bounds the wait
//! for S3's answer after the last byte as well as the upload itself. The answer
//! to [`ObjectStore::get`] is held to the deadline up to its head, and its body,
//! as large as the object, only by the SDK's stalled-stream protection: a
//! transfer that stops moving is the failure, not one that takes long. The copy
//! a trash makes is neither, and has [`SERVER_SIDE_COPY_DEADLINE`].
//!
//! The SDK takes its timeouts from one client-wide configuration, and a
//! client-wide deadline on the whole of an operation would cut off a large
//! upload as surely as it ends a stalled listing. So the client states the
//! bounds every call shares ([`timeout_config`], [`stalled_stream_protection`])
//! and leaves the operation deadlines explicitly off, and each call puts its
//! own on as it is sent (`within`). Where the client is built is the caller's —
//! the device builds it — and these are what it builds it with, so the numbers
//! are written down here, once, beside the calls they bound.
//!
//! [`ObjectStore::put`]: coffret_usecase::ObjectStore::put
//! [`ObjectStore::put_if_absent`]: coffret_usecase::ObjectStore::put_if_absent
//! [`ObjectStore::get`]: coffret_usecase::ObjectStore::get
//! [`Error::Timeout`]: coffret_usecase::Error::Timeout

use std::time::Duration;

use aws_sdk_s3::config::timeout::TimeoutConfig;
use aws_sdk_s3::config::StalledStreamProtectionConfig;

/// How long to wait for a connection to be established.
///
/// The Drive gateway's figure (`CONNECT_TIMEOUT` in its transport), so an
/// endpoint nothing is listening at is given up on alike whichever provider it
/// was. Inside [`SMALL_CALL_DEADLINE`] with room to spare, so a connection that
/// takes the whole of it still leaves a small call time to be answered.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

/// How long a transfer may go without moving a byte before it is a failure.
///
/// Watches the body of a transfer in either direction. On an upload it ends a
/// body that stops moving long before the deadline scaled to its length would;
/// on the answer to a `get` it is the only bound on the body, deliberately,
/// because a large Container is legitimately slow to arrive and only a stalled
/// one is a failure. It is the Drive gateway's between-bytes timeout
/// (`READ_TIMEOUT` in its transport) under the name the SDK gives the same
/// idea — a grace period after which zero throughput is a stalled stream.
pub const STALL_GRACE_PERIOD: Duration = Duration::from_secs(60);

/// How long a call whose body is small may take altogether.
///
/// A listing page, a `HEAD`, a delete, or the probes a Library's first
/// moments make: their answers are a page of keys at most, so a call that has
/// not finished in this long is not slow but stalled — an endpoint that took the
/// connection and says nothing, or an answer trickling a byte at a time. Neither
/// trips [`STALL_GRACE_PERIOD`], which watches the bodies of transfers and not
/// the wait for an answer to begin, and without this both would hold the call
/// forever, out of reach of the retry policy, which only acts on failures that
/// arrive.
///
/// Thirty seconds is past [`CONNECT_TIMEOUT`] with ten to spare for the answer,
/// and it is the Drive gateway's figure (`WHOLE_CALL_DEADLINE` in its
/// transport), so a slow Storage is judged alike whichever provider it is. It
/// is half the server's minute-long start-up catch-up wait
/// (`catch_up_at_startup::DEADLINE` in `coffret-server`, which the e2e suite's
/// start-up wait is measured against): a call that stalls while the explorer is
/// catching up is given up on, and made again, inside that minute rather than
/// outliving it.
///
/// It covers every attempt the SDK makes of the call, not each one: the SDK's
/// own retries are for answers that come back quickly and refused, and a call
/// that stalled is left to the retry policy above this crate, as it is on Drive,
/// whose transport makes no attempt twice.
pub const SMALL_CALL_DEADLINE: Duration = Duration::from_secs(30);

/// The slowest upload a call carrying a body is still waited out at, in bytes a
/// second.
///
/// A call that sends a body cannot be held to a fixed deadline: whatever figure
/// was chosen, some legitimate body would be large enough to outlast it on a
/// slow link — a Container is as large as the files it carries, and even a
/// conditional create's control object grows with the Library, since the Index
/// Snapshot a checkpoint writes is one. Nor can it go unbounded: once the last
/// byte is sent, the stalled-stream protection has nothing left to watch, and
/// an S3 that took the body and never answers would hold the call forever. A
/// floor on throughput is the bound that scales with the body, so such a call
/// is given [`SMALL_CALL_DEADLINE`] plus the time its body takes at this rate:
/// a small body is held to the deadline itself, and a large one to the time it
/// would take on the slowest link worth syncing over. Half a megabit a second
/// is below any such link; a Journal record of a few kilobytes adds nothing
/// measurable, a snapshot of several megabytes adds minutes, and a Container at
/// the single-request cap adds about a day.
///
/// Drive needs no counterpart: its uploads go out through a resumable session,
/// the call opening it is a small call, and the one carrying the body is held
/// between bytes by its transport.
pub const SLOWEST_UPLOAD_RATE: u64 = 64 * 1024;

/// How long the copy a trash makes may take altogether.
///
/// S3 has no trash, so trashing an object copies it into a reserved segment of
/// the key space and deletes the original. The copy is made inside the bucket
/// and moves none of the object's bytes over this connection, but S3 takes as
/// long over it as the object is large, and may hold the answer until it is
/// done — so it is not a small call, and [`SMALL_CALL_DEADLINE`] would cut off
/// the copy of a large Container and then every retry of it. Nor is it a
/// transfer the stalled-stream protection watches, since nothing streams. So
/// it has a deadline of its own, sized to the largest object a single request
/// can have stored ([`SINGLE_REQUEST_MAX_BYTES`], five gigabytes): at ten megabytes a
/// second, far below what S3 copies at, that is under nine minutes. It is still
/// a deadline: a copy nothing answers ends, and arrives as a timeout.
///
/// [`SINGLE_REQUEST_MAX_BYTES`]: crate::SINGLE_REQUEST_MAX_BYTES
pub const SERVER_SIDE_COPY_DEADLINE: Duration = Duration::from_secs(10 * 60);

/// The timeouts every call shares, for the client to be built with.
///
/// The connection is bounded by [`CONNECT_TIMEOUT`]. The three the SDK would
/// otherwise put on the whole of a call are off, and explicitly so, rather than
/// left to whatever the SDK's behavior version defaults them to: the SDK's read
/// timeout runs from the start of the request to the first byte of the answer,
/// which on an upload includes the whole of the body, and the two operation
/// timeouts bound the whole of the call. All three would cut off a large
/// upload; the small calls put their own on through `within`.
pub fn timeout_config() -> TimeoutConfig {
    TimeoutConfig::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .disable_read_timeout()
        .disable_operation_timeout()
        .disable_operation_attempt_timeout()
        .build()
}

/// The stalled-stream protection every call shares, for the client to be built
/// with: on, in both directions, with a grace period of [`STALL_GRACE_PERIOD`].
pub fn stalled_stream_protection() -> StalledStreamProtectionConfig {
    StalledStreamProtectionConfig::enabled()
        .grace_period(STALL_GRACE_PERIOD)
        .build()
}

/// A per-call override holding the whole of one call to `deadline`.
///
/// Both the operation and each attempt of it, so that neither the SDK's retries
/// nor one silent attempt can outlast it. Everything else — the connection
/// timeout, the stalled-stream protection — is left as the client has it: the
/// SDK merges an override's timeouts into the client's rather than replacing
/// them.
pub(crate) fn within(deadline: Duration) -> aws_sdk_s3::config::Builder {
    aws_sdk_s3::config::Builder::new().timeout_config(
        TimeoutConfig::builder()
            .operation_timeout(deadline)
            .operation_attempt_timeout(deadline)
            .build(),
    )
}

/// The deadline for a call sending a body of `len` bytes: `deadline`, and the
/// time the body takes at [`SLOWEST_UPLOAD_RATE`].
///
/// The one formula both calls that send a body — [`ObjectStore::put`] and
/// [`ObjectStore::put_if_absent`] — are held to; why it is a floor on
/// throughput rather than a fixed figure is on [`SLOWEST_UPLOAD_RATE`].
///
/// [`ObjectStore::put`]: coffret_usecase::ObjectStore::put
/// [`ObjectStore::put_if_absent`]: coffret_usecase::ObjectStore::put_if_absent
pub(crate) fn for_body_of(deadline: Duration, len: u64) -> Duration {
    deadline.saturating_add(Duration::from_secs(len / SLOWEST_UPLOAD_RATE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_body_is_held_to_the_small_call_deadline() {
        assert_eq!(
            for_body_of(SMALL_CALL_DEADLINE, 4 * 1024),
            SMALL_CALL_DEADLINE
        );
    }

    #[test]
    fn a_large_body_is_given_the_time_it_takes_at_the_floor_rate() {
        let snapshot = 64 * 1024 * 1024;
        assert_eq!(
            for_body_of(SMALL_CALL_DEADLINE, snapshot),
            SMALL_CALL_DEADLINE + Duration::from_secs(1024)
        );
    }

    #[test]
    fn the_client_states_every_timeout_rather_than_inheriting_one() {
        let config = timeout_config();
        assert_eq!(config.connect_timeout(), Some(CONNECT_TIMEOUT));
        assert_eq!(config.read_timeout(), None);
        assert_eq!(config.operation_timeout(), None);
        assert_eq!(config.operation_attempt_timeout(), None);
    }
}
