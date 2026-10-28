//! A number drawn onto an image, so that a page says which page it is.
//!
//! Drawn from a bitmap of its own rather than from a font: three by five cells
//! per digit is enough to read at any size a reader shows a page at, and it
//! keeps the generator free of a font file and of a crate to rasterise one.

use image::{Rgb, RgbImage};

/// Each digit as five rows of three cells, the high bit of each row the
/// leftmost cell.
const GLYPHS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], // 0
    [0b010, 0b110, 0b010, 0b010, 0b111], // 1
    [0b111, 0b001, 0b111, 0b100, 0b111], // 2
    [0b111, 0b001, 0b111, 0b001, 0b111], // 3
    [0b101, 0b101, 0b111, 0b001, 0b001], // 4
    [0b111, 0b100, 0b111, 0b001, 0b111], // 5
    [0b111, 0b100, 0b111, 0b101, 0b111], // 6
    [0b111, 0b001, 0b010, 0b010, 0b010], // 7
    [0b111, 0b101, 0b111, 0b101, 0b111], // 8
    [0b111, 0b101, 0b111, 0b001, 0b111], // 9
];

const GLYPH_WIDTH: u32 = 3;
const GLYPH_HEIGHT: u32 = 5;
/// The cells between two digits, and around the digits on the plate.
const GAP: u32 = 1;

const PLATE: Rgb<u8> = Rgb([255, 255, 255]);
const INK: Rgb<u8> = Rgb([200, 30, 30]);

/// Draws `number` in the middle of `image`, dark red on a white plate, as
/// large as fits in about two thirds of the width and a third of the height.
///
/// The plate is what keeps it legible over whatever the page already has on
/// it, and the red is what tells it apart from the page's own grey "text".
/// Nothing is drawn past the image's edges, however small the image is.
pub fn stamp(image: &mut RgbImage, number: usize) {
    let digits: Vec<usize> = number
        .to_string()
        .bytes()
        .map(|digit| usize::from(digit - b'0'))
        .collect();
    let count = digits.len() as u32;
    // The plate's size in cells: the digits with a gap between each two, and
    // a gap all round.
    let cells_wide = count * GLYPH_WIDTH + (count - 1) * GAP + 2 * GAP;
    let cells_high = GLYPH_HEIGHT + 2 * GAP;

    let (width, height) = image.dimensions();
    let cell = (width * 2 / 3 / cells_wide)
        .min(height / 3 / cells_high)
        .max(1);
    let plate_width = cells_wide * cell;
    let plate_height = cells_high * cell;
    let left = width.saturating_sub(plate_width) / 2;
    let top = height.saturating_sub(plate_height) / 2;

    let mut fill = |x0: u32, y0: u32, w: u32, h: u32, color: Rgb<u8>| {
        for y in y0..(y0 + h).min(height) {
            for x in x0..(x0 + w).min(width) {
                image.put_pixel(x, y, color);
            }
        }
    };

    fill(left, top, plate_width, plate_height, PLATE);
    for (position, digit) in digits.iter().enumerate() {
        let glyph_left = left + (GAP + position as u32 * (GLYPH_WIDTH + GAP)) * cell;
        let glyph_top = top + GAP * cell;
        for (row, bits) in GLYPHS[*digit].iter().enumerate() {
            for column in 0..GLYPH_WIDTH {
                if bits & (1 << (GLYPH_WIDTH - 1 - column)) != 0 {
                    fill(
                        glyph_left + column * cell,
                        glyph_top + row as u32 * cell,
                        cell,
                        cell,
                        INK,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blank(width: u32, height: u32) -> RgbImage {
        RgbImage::from_pixel(width, height, Rgb([0, 0, 0]))
    }

    fn inked(image: &RgbImage) -> usize {
        image.pixels().filter(|pixel| **pixel == INK).count()
    }

    #[test]
    fn draws_ink_on_a_plate() {
        let mut image = blank(120, 180);
        stamp(&mut image, 7);
        assert!(inked(&image) > 0, "the digit is drawn");
        assert!(
            image.pixels().any(|pixel| *pixel == PLATE),
            "the digit stands on a plate"
        );
    }

    #[test]
    fn different_numbers_draw_differently() {
        let draw = |number| {
            let mut image = blank(120, 180);
            stamp(&mut image, number);
            image
        };
        assert_ne!(draw(1), draw(7));
        assert_ne!(draw(12), draw(21));
        assert_eq!(draw(300), draw(300));
    }

    #[test]
    fn stays_inside_an_image_too_small_for_it() {
        // Three digits need more than four pixels across; what does not fit is
        // cut off rather than drawn past the edge.
        let mut image = blank(4, 3);
        stamp(&mut image, 300);
        assert_eq!(image.dimensions(), (4, 3));
    }
}
