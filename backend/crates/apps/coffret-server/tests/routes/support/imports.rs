//! Dropping files by their paths on this device, as the desktop app's window
//! does: the app walks them and holds what it took, and the page imports it.

use std::path::Path;

use axum::body::Body;
use axum::http::Response;
use coffret_device::{walk_drop, Dropped};
use serde_json::{json, Value};

use super::Served;

impl Served {
    /// Walks the items at `paths` and holds what was taken, as the desktop
    /// app does with what was let go of over its window.
    pub async fn hold_drop(&self, paths: &[&Path]) -> Dropped {
        let paths: Vec<String> = paths
            .iter()
            .map(|path| path.to_str().expect("a case drops text paths").to_owned())
            .collect();
        self.state
            .drops
            .hold(walk_drop(&paths).await)
            .expect("a handle is drawn")
    }

    /// Imports every file `dropped` holds into `folder` under the names it
    /// was told them by, as the page does with a drop it renamed nothing in.
    pub async fn import_all(
        &self,
        folder: &str,
        dropped: &Dropped,
        freeze: bool,
    ) -> Response<Body> {
        let files: Vec<Value> = dropped
            .files
            .iter()
            .enumerate()
            .map(|(index, row)| json!({ "index": index, "name": row.path }))
            .collect();
        self.import(&json!({
            "path": folder,
            "freeze": freeze,
            "handle": dropped.handle.as_deref().expect("the drop took something"),
            "files": files,
        }))
        .await
    }

    /// Sends one import, as the page sends it.
    pub async fn import(&self, body: &Value) -> Response<Body> {
        self.post_json("/api/import", body).await
    }
}
