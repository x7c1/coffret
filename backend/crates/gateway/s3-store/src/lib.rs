//! [`ObjectStore`](coffret_usecase::ObjectStore) over an S3 bucket.
//!
//! S3 keys objects by name, so most of the port maps straight onto it: a name
//! is a key, a reference is a name, and a commit slot is nothing at all because
//! the key space already reserves itself. Three places need work, and all of
//! them are here rather than in any caller:
//!
//! - **Conditional create.** [`ObjectStore::put_if_absent`] is a PUT carrying
//!   `If-None-Match: *`, so of several writers aiming at one key exactly one is
//!   stored and the rest are refused rather than overwriting.
//! - **A trash.** S3 has none, so [`ObjectStore::trash`] moves the object into a
//!   reserved segment of the key space and [`ObjectStore::purge`] clears both
//!   segments and reads back to confirm it.
//! - **A cap on one request.** Every write goes out as a single `PutObject`,
//!   which S3 caps at [`SINGLE_REQUEST_MAX_BYTES`], so a larger body is refused
//!   before a byte of it is sent rather than after all of it has travelled.
//!   Multipart upload is what lifts the cap, and this gateway does not do it
//!   yet.
//!
//! One call sits outside the port entirely. [`check_bucket`] asks whether a
//! bucket is there at all, which is the question creating or joining a Library
//! has to put to Storage before there is a store to put anything to — on S3 a
//! prefix exists only by being written under, so nothing else would ask until
//! the first sync. It is here rather than with its caller for the same reason
//! everything below is: reading the answer means reading a status and an S3
//! error code, and there is one table for that.
//!
//! Failures come back in the port's vocabulary: nothing above this crate sees
//! an S3 error code, and a caller decides what to do from the variant rather
//! than from a message.
//!
//! The `aws_sdk_s3::Client` is a constructor argument, which is what lets the
//! conformance suite run this gateway against a MinIO container without the
//! gateway knowing anything but S3.
//!
//! ```no_run
//! use coffret_usecase::{ByteStream, ObjectStore};
//! use s3_store::{S3Settings, S3};
//!
//! # async fn example(client: aws_sdk_s3::Client) -> coffret_usecase::Result<()> {
//! // The prefix is the Library's app folder as a key prefix: the base the user
//! // chose, with `coffret-<library id>/` after it (spec: FM-18).
//! let prefix = "photos/coffret-0123456789abcdef/";
//! let store = S3::new(client, S3Settings::new("my-bucket").with_prefix(prefix));
//! // Containers carry opaque names; the recognizable ones are control objects'.
//! let name = "0123456789abcdef0123456789abcdef.cfrt";
//! let object = store.put(name, ByteStream::from(b"ciphertext".to_vec())).await?;
//! let bytes = store.get(&object, None).await?.into_bytes().await?;
//! # Ok(())
//! # }
//! ```
//!
//! [`ObjectStore::put_if_absent`]: coffret_usecase::ObjectStore::put_if_absent
//! [`ObjectStore::trash`]: coffret_usecase::ObjectStore::trash
//! [`ObjectStore::purge`]: coffret_usecase::ObjectStore::purge

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod check_bucket;
pub use check_bucket::check_bucket;

mod error;

mod key_layout;

mod reader_body;

mod s3;
pub use s3::S3;

mod settings;
pub use settings::S3Settings;

mod single_request_limit;
pub use single_request_limit::SINGLE_REQUEST_MAX_BYTES;
