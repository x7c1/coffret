//! Refuses to embed an explorer build that holds a symbolic link.
//!
//! Only under `embed-web`, which is the only build that embeds anything.

#[path = "build/dist_links.rs"]
mod dist_links;

use std::path::Path;

/// Where `make web-dist` writes the explorer's build output, relative to this
/// crate: the directory `src/page` embeds.
const DIST: &str = "../../../../frontend/packages/apps/web/dist";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build/dist_links.rs");
    if std::env::var_os("CARGO_FEATURE_EMBED_WEB").is_none() {
        return;
    }
    println!("cargo:rerun-if-changed={DIST}");
    let links = match dist_links::links_in(Path::new(DIST)) {
        Ok(links) => links,
        Err(cause) => panic!("the explorer's build output at {DIST} could not be read: {cause}"),
    };
    if !links.is_empty() {
        panic!(
            "the explorer's build output holds symbolic links, which would embed whatever they \
             point at: {links:?}"
        );
    }
}
