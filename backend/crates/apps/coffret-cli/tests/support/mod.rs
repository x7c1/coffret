//! Driving the built binary the way a person does.
//!
//! Every case here runs `coffret` itself rather than calling into
//! `coffret-device`, because what is being checked is the shell: that the flags
//! parse into the call they claim to, that what a caller might pipe reaches
//! standard output and nothing else does, and that a run's exit status says
//! which of the three things happened to it.

// Every test target compiles this module for itself, so what one of them does
// not reach for is dead code to that one and not to the other — and a
// re-export it does not reach for is an unused import for the same reason.
#![allow(dead_code, unused_imports)]

/// The Passphrase every case here uses.
pub const PASSPHRASE: &str = "correct horse battery staple";

/// What a Recovery Code starts with, printed or not (spec: KD-11).
pub const RECOVERY_CODE_PREFIX: &str = "coffret1";

/// The region every case signs for.
pub const REGION: &str = "us-east-1";

/// What a run that succeeded but left findings exits with.
pub const FINDINGS: i32 = 2;

mod device;
pub use device::{write_file, Device};

mod minio;
pub use minio::{minio, Minio};

mod output;
pub use output::{code, printed_code, printed_prefix, stderr, stdout, succeeded};

mod stub_bucket;
pub use stub_bucket::stub_endpoint;
