//! The `ObjectStore` conformance suite, run against a real S3 implementation.
//!
//! The suite itself lives with the port; what is here is only the wiring that
//! points it at a bucket, and even that lives in `minio`, because the commit
//! suite next door needs the same thing. `make s3-store-it` starts MinIO in
//! Docker, sets the environment it reads, runs this target, and tears the
//! container down again. Without that environment the cases report themselves
//! skipped, so an ordinary `cargo test` neither needs Docker nor pretends to
//! have covered S3.
//!
//! A configured run writes every call it makes to a file under
//! `$XDG_STATE_HOME/coffret/logs` — `$HOME/.local/state/coffret/logs` where that
//! is unset — and prints the name of it as it starts. This gateway is driven by
//! nothing else in the workspace, so without that sink everything it records
//! would be emitted into nothing. `COFFRET_LOG_DIR` moves the file and
//! `COFFRET_LOG_MAX_BYTES` bounds how much is kept. It is JSONL — one JSON
//! object per line, the fields each call was recorded with kept as fields — so
//! it is read with `jq` rather than an eye; `make s3-store-it` carries the
//! recipe.

use coffret_usecase::conformance::StoreUnderTest;

mod minio;

/// Hands the suite an empty store, or `None` when no endpoint is configured.
async fn fixture() -> Option<StoreUnderTest> {
    let (store, page_size) = minio::store("conformance").await?;
    Some(StoreUnderTest::new(Box::new(store), page_size))
}

coffret_usecase::object_store_conformance!(fixture().await);
