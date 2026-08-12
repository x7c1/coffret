//! Viewer-performance spike server: serves a plain image folder as a library
//! (no encryption yet — this measures whether the browser-based viewer holds
//! up at thousands of images before the E2EE pipeline is built underneath).

use anyhow::{Context, Result};
use axum::extract::{Path, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use clap::Parser;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Parser)]
struct Args {
    /// Root folder whose images become the library
    #[arg(long)]
    library: PathBuf,

    /// Directory for cached thumbnails
    #[arg(long, default_value = ".tmp/thumbs")]
    thumbs: PathBuf,

    /// Listen port
    #[arg(long, default_value_t = 8787)]
    port: u16,
}

struct LibraryFile {
    rel: String,
    abs: PathBuf,
}

struct App {
    files: Vec<LibraryFile>,
    thumbs: PathBuf,
}

#[derive(Serialize)]
struct EntryDto {
    id: usize,
    path: String,
}

const IMAGE_EXTENSIONS: [&str; 4] = ["jpg", "jpeg", "png", "webp"];
const THUMBNAIL_MAX_HEIGHT: u32 = 256;
const THUMBNAIL_JPEG_QUALITY: u8 = 80;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let files = scan_library(&args.library)?;
    println!(
        "library: {} files under {}",
        files.len(),
        args.library.display()
    );

    std::fs::create_dir_all(&args.thumbs).with_context(|| {
        format!(
            "cannot create thumbnail cache dir {}",
            args.thumbs.display()
        )
    })?;

    let app = Arc::new(App {
        files,
        thumbs: args.thumbs,
    });
    let router = Router::new()
        .route("/api/entries", get(list_entries))
        .route("/api/image/{id}", get(serve_image))
        .route("/api/thumb/{id}", get(serve_thumb))
        .with_state(app);

    let addr = format!("127.0.0.1:{}", args.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("cannot bind {addr}"))?;
    println!("listening on http://{addr}");
    axum::serve(listener, router).await?;
    Ok(())
}

/// Collects image files under `root`, sorted by relative path so entry ids are
/// stable for a given library state.
fn scan_library(root: &std::path::Path) -> Result<Vec<LibraryFile>> {
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry.with_context(|| format!("cannot scan {}", root.display()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let ext = entry
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase);
        let is_image = ext
            .as_deref()
            .is_some_and(|e| IMAGE_EXTENSIONS.contains(&e));
        if !is_image {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .expect("walkdir yields paths under root")
            .to_string_lossy()
            .into_owned();
        files.push(LibraryFile {
            rel,
            abs: entry.path().to_path_buf(),
        });
    }
    files.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(files)
}

async fn list_entries(State(app): State<Arc<App>>) -> Json<Vec<EntryDto>> {
    let entries = app
        .files
        .iter()
        .enumerate()
        .map(|(id, f)| EntryDto {
            id,
            path: f.rel.clone(),
        })
        .collect();
    Json(entries)
}

async fn serve_image(State(app): State<Arc<App>>, Path(id): Path<usize>) -> Response {
    let Some(file) = app.files.get(id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match tokio::fs::read(&file.abs).await {
        Ok(bytes) => image_response(content_type_of(&file.rel), bytes),
        Err(e) => internal_error(format!("cannot read {}: {e}", file.rel)),
    }
}

async fn serve_thumb(State(app): State<Arc<App>>, Path(id): Path<usize>) -> Response {
    let Some(file) = app.files.get(id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let cache_path = app.thumbs.join(format!("{id}.jpg"));
    match tokio::fs::read(&cache_path).await {
        Ok(bytes) => return image_response("image/jpeg", bytes),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return internal_error(format!("cannot read thumbnail cache: {e}")),
    }

    let source = file.abs.clone();
    let generated =
        tokio::task::spawn_blocking(move || generate_thumbnail(&source, &cache_path)).await;
    match generated {
        Ok(Ok(bytes)) => image_response("image/jpeg", bytes),
        Ok(Err(e)) => internal_error(format!("cannot generate thumbnail for {}: {e:#}", file.rel)),
        Err(e) => internal_error(format!("thumbnail task panicked: {e}")),
    }
}

/// Decodes the source image, scales it down, and writes it to the cache via a
/// unique temp file + rename so concurrent requests never observe a torn file.
fn generate_thumbnail(source: &std::path::Path, cache_path: &std::path::Path) -> Result<Vec<u8>> {
    static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

    let image =
        image::open(source).with_context(|| format!("cannot decode {}", source.display()))?;
    let thumb = image.thumbnail(u32::MAX, THUMBNAIL_MAX_HEIGHT);
    let mut bytes = Vec::new();
    let encoder =
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, THUMBNAIL_JPEG_QUALITY);
    thumb.to_rgb8().write_with_encoder(encoder)?;

    let temp =
        cache_path.with_extension(format!("tmp{}", TEMP_SEQ.fetch_add(1, Ordering::Relaxed)));
    std::fs::write(&temp, &bytes)?;
    std::fs::rename(&temp, cache_path)?;
    Ok(bytes)
}

fn content_type_of(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => "image/jpeg",
    }
}

fn image_response(content_type: &'static str, bytes: Vec<u8>) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            // Entries are immutable for a given library state; let the browser
            // cache aggressively so scroll-back never refetches.
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        bytes,
    )
        .into_response()
}

fn internal_error(message: String) -> Response {
    eprintln!("error: {message}");
    (StatusCode::INTERNAL_SERVER_ERROR, message).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_finds_images_recursively_and_sorts_by_path() {
        let dir = std::env::temp_dir().join(format!("coffret-scan-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("b-album")).unwrap();
        std::fs::create_dir_all(dir.join("a-album")).unwrap();
        std::fs::write(dir.join("b-album/2.jpg"), b"x").unwrap();
        std::fs::write(dir.join("a-album/1.PNG"), b"x").unwrap();
        std::fs::write(dir.join("a-album/note.txt"), b"x").unwrap();

        let files = scan_library(&dir).unwrap();
        let rels: Vec<&str> = files.iter().map(|f| f.rel.as_str()).collect();
        assert_eq!(rels, ["a-album/1.PNG", "b-album/2.jpg"]);

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn content_type_follows_extension() {
        assert_eq!(content_type_of("a/b.png"), "image/png");
        assert_eq!(content_type_of("a/b.webp"), "image/webp");
        assert_eq!(content_type_of("a/b.jpeg"), "image/jpeg");
    }
}
