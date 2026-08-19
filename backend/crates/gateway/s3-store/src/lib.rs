//! [`ObjectStore`](coffret_usecase::ObjectStore) over an S3 bucket.
//!
//! S3 keys objects by name, so most of the port maps straight onto it: a name
//! is a key, a reference is a name, and a commit slot is nothing at all because
//! the key space already reserves itself. Two places need work, and both are
//! here rather than in any caller:
//!
//! - **Conditional create.** [`ObjectStore::put_if_absent`] is a PUT carrying
//!   `If-None-Match: *`, so of several writers aiming at one key exactly one is
//!   stored and the rest are refused rather than overwriting.
//! - **A trash.** S3 has none, so [`ObjectStore::trash`] moves the object into a
//!   reserved segment of the key space and [`ObjectStore::purge`] clears both
//!   segments and reads back to confirm it.
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
//! let store = S3::new(client, S3Settings::new("my-bucket").with_prefix("libraries/alpha"));
//! let object = store.put("jrn-1.cfrt", ByteStream::from(b"ciphertext".to_vec())).await?;
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

mod error;

mod key_layout;

mod reader_body;

mod s3;
pub use s3::S3;

mod settings;
pub use settings::S3Settings;
