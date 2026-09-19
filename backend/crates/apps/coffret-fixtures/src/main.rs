//! Generates a synthetic library of JPEGs: photo albums plus one scanned-book
//! folder, so a run has files to work on without touching personal data.
//!
//! The image sizes are arguments, because what a caller needs out of them
//! differs: the viewer benchmark wants camera-sized images, since realistic
//! decode cost is the thing it measures, while a round trip against a real
//! Storage only needs files that differ from one another. The defaults are the
//! camera-sized ones.

use anyhow::{Context, Result};
use clap::Parser;
use image::RgbImage;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::str::FromStr;

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

    /// Photo dimensions in pixels, as WIDTHxHEIGHT
    #[arg(long, default_value_t = Size::new(1600, 1200))]
    photo_size: Size,

    /// Book page dimensions in pixels, as WIDTHxHEIGHT
    #[arg(long, default_value_t = Size::new(1200, 1800))]
    page_size: Size,
}

const PHOTOS_PER_ALBUM: usize = 100;
const JPEG_QUALITY: u8 = 85;

/// The dimensions of one generated image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Size {
    width: u32,
    height: u32,
}

impl Size {
    fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// The smaller side, which is what the drawing below scales against: a
    /// feature sized off the longer side alone would run off the shorter one.
    fn shorter_side(self) -> u32 {
        self.width.min(self.height)
    }
}

impl std::fmt::Display for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}", self.width, self.height)
    }
}

impl FromStr for Size {
    /// A string rather than an error type of its own: clap prints it as it is,
    /// and nothing here catches one to act on it.
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let malformed = || format!("expected a size as WIDTHxHEIGHT in pixels, got `{text}`");
        let (width, height) = text.split_once('x').ok_or_else(malformed)?;
        let width: u32 = width.parse().map_err(|_| malformed())?;
        let height: u32 = height.parse().map_err(|_| malformed())?;
        if width == 0 || height == 0 {
            return Err(format!(
                "expected a size as WIDTHxHEIGHT in pixels, both sides above zero, got `{text}`"
            ));
        }
        Ok(Self::new(width, height))
    }
}

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
        .try_for_each(|(path, i)| save_jpeg(path, photo(*i, args.photo_size)))?;
    page_jobs
        .par_iter()
        .try_for_each(|(path, i)| save_jpeg(path, page(*i, args.page_size)))?;

    println!(
        "generated {} photos at {} + {} pages at {} under {}",
        args.photos,
        args.photo_size,
        args.pages,
        args.page_size,
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
    /// Wrapping rather than plain arithmetic: every seed above two overflows
    /// the multiply, which a release build wraps silently and a debug build —
    /// the one the tests run under — panics on.
    fn new(seed: usize) -> Self {
        Self(
            (seed as u64)
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407),
        )
    }

    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
}

/// A "photo": two-color gradient plus a scatter of solid blocks. Enough
/// structure to give JPEG decode a realistic cost at camera sizes.
///
/// The blocks are sized as a fraction of the shorter side: 40..300 pixels at
/// the default 1600x1200, and a handful of pixels on a canvas a few dozen
/// pixels a side, rather than something that would cover it.
fn photo(index: usize, size: Size) -> RgbImage {
    let Size {
        width: w,
        height: h,
    } = size;
    let mut rng = Lcg::new(index);
    let top = [rng.next() as u8, rng.next() as u8, rng.next() as u8];
    let bottom = [rng.next() as u8, rng.next() as u8, rng.next() as u8];

    let mut img = RgbImage::from_fn(w, h, |_, y| {
        let t = y as f32 / h as f32;
        let mix = |a: u8, b: u8| (a as f32 * (1.0 - t) + b as f32 * t) as u8;
        image::Rgb([
            mix(top[0], bottom[0]),
            mix(top[1], bottom[1]),
            mix(top[2], bottom[2]),
        ])
    });

    let smallest = (size.shorter_side() / 30).max(1);
    let spread = (size.shorter_side() * 13 / 60).max(1);
    for _ in 0..24 {
        let color = image::Rgb([rng.next() as u8, rng.next() as u8, rng.next() as u8]);
        let bw = smallest + rng.next() % spread;
        let bh = smallest + rng.next() % spread;
        // A block never covers the whole canvas — it is at most a quarter of
        // the shorter side — but the placement still guards against a zero
        // range, and the fill against a block that ends past the last pixel.
        let x0 = rng.next() % w.saturating_sub(bw).max(1);
        let y0 = rng.next() % h.saturating_sub(bh).max(1);
        for y in y0..(y0 + bh).min(h) {
            for x in x0..(x0 + bw).min(w) {
                img.put_pixel(x, y, color);
            }
        }
    }
    img
}

/// A scanned-book "page": white background with dark text-like line runs.
///
/// Margins, line height and run lengths are fractions of the two sides. All
/// but the margins stop shrinking at a pixel, so the runs still fit at small
/// sizes.
fn page(index: usize, size: Size) -> RgbImage {
    let Size {
        width: w,
        height: h,
    } = size;
    let mut rng = Lcg::new(index + 1_000_000);
    let mut img = RgbImage::from_pixel(w, h, image::Rgb([245, 243, 238]));

    let margin_x = w / 12;
    let margin_y = h / 15;
    let line = (h * 7 / 450).max(1);
    let leading = (h / 100).max(1);
    let shortest_run = (w / 60).max(1);
    let run_spread = (w / 10).max(1);
    let shortest_gap = (w / 120).max(1);
    let gap_spread = (w / 40).max(1);

    let mut y = margin_y;
    while y + line < h - margin_y {
        let mut x = margin_x;
        while x + shortest_run < w - margin_x {
            let run = shortest_run + rng.next() % run_spread;
            let gap = shortest_gap + rng.next() % gap_spread;
            let ink = 20 + (rng.next() % 60) as u8;
            let x_end = (x + run).min(w - margin_x);
            for yy in y..y + line {
                for xx in x..x_end {
                    img.put_pixel(xx, yy, image::Rgb([ink, ink, ink]));
                }
            }
            x = x_end + gap;
        }
        y += line + leading;
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_width_by_height() {
        assert_eq!("1600x1200".parse(), Ok(Size::new(1600, 1200)));
        assert_eq!("64x48".parse(), Ok(Size::new(64, 48)));
    }

    #[test]
    fn rejects_anything_that_is_not_a_width_by_a_height() {
        for text in ["1600", "1600x1200x1", "1600x", "x1200", "1600X1200", "wide"] {
            let error = text.parse::<Size>().expect_err(text);
            assert!(
                error.contains("WIDTHxHEIGHT"),
                "the error for `{text}` must name the expected form: {error}"
            );
        }
    }

    #[test]
    fn rejects_a_side_of_zero() {
        for text in ["0x0", "0x1200", "1600x0"] {
            let error = text.parse::<Size>().expect_err(text);
            assert!(
                error.contains("above zero"),
                "the error for `{text}` must say why zero is refused: {error}"
            );
        }
    }

    #[test]
    fn draws_a_photo_at_a_small_size() {
        let size = Size::new(64, 48);
        let image = photo(0, size);
        assert_eq!(image.dimensions(), (size.width, size.height));
        assert!(
            image.pixels().any(|pixel| pixel != image.get_pixel(0, 0)),
            "a photo is a gradient with blocks on it, not one flat color"
        );
    }

    #[test]
    fn draws_a_page_at_a_small_size() {
        let size = Size::new(64, 96);
        let image = page(0, size);
        assert_eq!(image.dimensions(), (size.width, size.height));
        assert!(
            image
                .pixels()
                .any(|pixel| pixel.0 != [245, 243, 238] && pixel.0[0] < 120),
            "a page is ink on paper, not a blank sheet"
        );
    }

    #[test]
    fn the_same_index_and_size_draw_the_same_image() {
        let size = Size::new(80, 60);
        assert_eq!(photo(7, size), photo(7, size));
        assert_ne!(photo(7, size), photo(8, size));
        let size = Size::new(60, 90);
        assert_eq!(page(3, size), page(3, size));
        assert_ne!(page(3, size), page(4, size));
    }

    #[test]
    fn writes_a_jpeg_that_reads_back_at_the_size_asked_for() {
        let dir = std::env::temp_dir().join(format!(
            "coffret-fixtures-test-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&dir).expect("the scratch directory is ours to make");
        let path = dir.join("img.jpg");

        save_jpeg(&path, photo(0, Size::new(64, 48))).expect("the photo is written");
        let written = std::fs::metadata(&path).expect("the file is there");
        assert!(written.len() > 0, "the encoder wrote nothing");
        let read_back = image::open(&path).expect("what was written is a JPEG");
        assert_eq!((read_back.width(), read_back.height()), (64, 48));

        std::fs::remove_dir_all(&dir).expect("the scratch directory is ours to remove");
    }
}
