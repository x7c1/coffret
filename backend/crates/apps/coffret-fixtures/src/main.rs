//! Generates a synthetic benchmark library for the viewer spike: photo albums
//! (camera-sized JPEGs) plus one scanned-book folder (page-sized JPEGs), so
//! viewer performance can be measured at realistic decode cost without using
//! personal data.

use anyhow::{Context, Result};
use clap::Parser;
use image::RgbImage;
use rayon::prelude::*;
use std::path::{Path, PathBuf};

#[derive(Parser)]
struct Args {
    /// Output directory for the generated library
    #[arg(long)]
    out: PathBuf,

    /// Number of photos (split into albums of 100)
    #[arg(long, default_value_t = 3000)]
    photos: usize,

    /// Number of book pages (single book folder)
    #[arg(long, default_value_t = 300)]
    pages: usize,
}

const PHOTOS_PER_ALBUM: usize = 100;
const JPEG_QUALITY: u8 = 85;

fn main() -> Result<()> {
    let args = Args::parse();

    let photo_jobs: Vec<(PathBuf, usize)> = (0..args.photos)
        .map(|i| {
            let album = i / PHOTOS_PER_ALBUM;
            let path = args
                .out
                .join(format!("album-{album:03}"))
                .join(format!("img-{i:05}.jpg"));
            (path, i)
        })
        .collect();
    let page_jobs: Vec<(PathBuf, usize)> = (0..args.pages)
        .map(|i| {
            let path = args.out.join("book-000").join(format!("page-{i:04}.jpg"));
            (path, i)
        })
        .collect();

    for (path, _) in photo_jobs.iter().chain(&page_jobs) {
        let dir = path.parent().expect("jobs always have a parent dir");
        std::fs::create_dir_all(dir).with_context(|| format!("cannot create {}", dir.display()))?;
    }

    photo_jobs
        .par_iter()
        .try_for_each(|(path, i)| save_jpeg(path, photo(*i)))?;
    page_jobs
        .par_iter()
        .try_for_each(|(path, i)| save_jpeg(path, page(*i)))?;

    println!(
        "generated {} photos + {} pages under {}",
        args.photos,
        args.pages,
        args.out.display()
    );
    Ok(())
}

fn save_jpeg(path: &Path, image: RgbImage) -> Result<()> {
    let mut bytes = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY);
    image
        .write_with_encoder(encoder)
        .with_context(|| format!("cannot encode {}", path.display()))?;
    std::fs::write(path, bytes).with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

/// Deterministic per-index pseudo-randomness so runs are reproducible.
struct Lcg(u64);

impl Lcg {
    fn new(seed: usize) -> Self {
        Self(seed as u64 * 6364136223846793005 + 1442695040888963407)
    }

    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
}

/// A camera-sized "photo": two-color gradient plus a scatter of solid blocks.
/// Enough structure to give JPEG decode a realistic cost.
fn photo(index: usize) -> RgbImage {
    const W: u32 = 1600;
    const H: u32 = 1200;
    let mut rng = Lcg::new(index);
    let top = [rng.next() as u8, rng.next() as u8, rng.next() as u8];
    let bottom = [rng.next() as u8, rng.next() as u8, rng.next() as u8];

    let mut img = RgbImage::from_fn(W, H, |_, y| {
        let t = y as f32 / H as f32;
        let mix = |a: u8, b: u8| (a as f32 * (1.0 - t) + b as f32 * t) as u8;
        image::Rgb([
            mix(top[0], bottom[0]),
            mix(top[1], bottom[1]),
            mix(top[2], bottom[2]),
        ])
    });

    for _ in 0..24 {
        let color = image::Rgb([rng.next() as u8, rng.next() as u8, rng.next() as u8]);
        let bw = 40 + rng.next() % 260;
        let bh = 40 + rng.next() % 260;
        let x0 = rng.next() % (W - bw);
        let y0 = rng.next() % (H - bh);
        for y in y0..y0 + bh {
            for x in x0..x0 + bw {
                img.put_pixel(x, y, color);
            }
        }
    }
    img
}

/// A scanned-book "page": white background with dark text-like line runs.
fn page(index: usize) -> RgbImage {
    const W: u32 = 1200;
    const H: u32 = 1800;
    let mut rng = Lcg::new(index + 1_000_000);
    let mut img = RgbImage::from_pixel(W, H, image::Rgb([245, 243, 238]));

    let mut y = 120u32;
    while y + 28 < H - 120 {
        let mut x = 100u32;
        while x + 20 < W - 100 {
            let run = 20 + rng.next() % 120;
            let gap = 10 + rng.next() % 30;
            let ink = 20 + (rng.next() % 60) as u8;
            let x_end = (x + run).min(W - 100);
            for yy in y..y + 28 {
                for xx in x..x_end {
                    img.put_pixel(xx, yy, image::Rgb([ink, ink, ink]));
                }
            }
            x = x_end + gap;
        }
        y += 28 + 18;
    }
    img
}
