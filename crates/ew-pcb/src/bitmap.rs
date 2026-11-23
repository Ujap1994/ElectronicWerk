//! Black-and-white pictures (logos, QR codes) as filled board polygons.
//!
//! The outline of every set pixel region is followed along the pixel edges
//! (pixels that only touch at a corner are separate). A board polygon has
//! no holes, so each hole is joined to the outline of its region by a cut
//! of zero width (a "keyhole", which Gerber regions allow). The polygons
//! keep the pixel steps: at the sizes of logos on boards they are finer
//! than silkscreen or copper can show.

use std::collections::HashMap;

use ew_model::units::{Nm, Point};
use qrcodegen::{QrCode, QrCodeEcc};

/// A picture of set and clear pixels, row by row from the top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    width: usize,
    height: usize,
    bits: Vec<bool>,
}

/// Why a picture cannot be made.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BitmapError {
    /// The rows have different lengths or there are none.
    #[error("the picture must have rows of equal length")]
    Ragged,
    /// More pixels than allowed.
    #[error("the picture is too large ({0} pixels; at most {MAX_PIXELS})")]
    TooLarge(usize),
    /// A row has a character other than the ones for set and clear pixels.
    #[error("unexpected character `{0}` in a row (use `#` or `1` for set, `.` or `0` for clear)")]
    Character(char),
    /// The text does not fit into a QR code.
    #[error("the text is too long for a QR code")]
    TooLong,
}

/// The largest picture (pixels).
pub const MAX_PIXELS: usize = 4_000_000;

impl Bitmap {
    /// A clear picture.
    #[must_use]
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            bits: vec![false; width * height],
        }
    }

    /// A picture from rows of `#`/`1` (set) and `.`/`0`/space (clear).
    ///
    /// # Errors
    ///
    /// When the rows differ in length, are too many or hold other
    /// characters.
    pub fn from_rows<S: AsRef<str>>(rows: &[S]) -> Result<Self, BitmapError> {
        let width = rows.first().map_or(0, |r| r.as_ref().chars().count());
        if width == 0 || rows.iter().any(|r| r.as_ref().chars().count() != width) {
            return Err(BitmapError::Ragged);
        }
        let pixels = width.saturating_mul(rows.len());
        if pixels > MAX_PIXELS {
            return Err(BitmapError::TooLarge(pixels));
        }
        let mut bits = Vec::with_capacity(pixels);
        for row in rows {
            for c in row.as_ref().chars() {
                bits.push(match c {
                    '#' | '1' | 'X' | 'x' => true,
                    '.' | '0' | ' ' | '-' => false,
                    other => return Err(BitmapError::Character(other)),
                });
            }
        }
        Ok(Self {
            width,
            height: rows.len(),
            bits,
        })
    }

    /// The modules of a QR code for a text, with `quiet` clear modules
    /// around it.
    ///
    /// # Errors
    ///
    /// When the text is too long.
    pub fn qr(text: &str, ecc: QrEcc, quiet: usize) -> Result<Self, BitmapError> {
        let level = match ecc {
            QrEcc::Low => QrCodeEcc::Low,
            QrEcc::Medium => QrCodeEcc::Medium,
            QrEcc::Quartile => QrCodeEcc::Quartile,
            QrEcc::High => QrCodeEcc::High,
        };
        let code = QrCode::encode_text(text, level).map_err(|_| BitmapError::TooLong)?;
        let modules = usize::try_from(code.size()).unwrap_or(0);
        let size = modules + 2 * quiet;
        let mut out = Self::new(size, size);
        for y in 0..modules {
            for x in 0..modules {
                let (cx, cy) = (i32::try_from(x).unwrap_or(0), i32::try_from(y).unwrap_or(0));
                out.set(x + quiet, y + quiet, code.get_module(cx, cy));
            }
        }
        Ok(out)
    }

    /// Width in pixels.
    #[must_use]
    pub const fn width(&self) -> usize {
        self.width
    }

    /// Height in pixels.
    #[must_use]
    pub const fn height(&self) -> usize {
        self.height
    }

    /// Whether a pixel is set (outside: clear).
    #[must_use]
    pub fn get(&self, x: i64, y: i64) -> bool {
        if x < 0 || y < 0 {
            return false;
        }
        let (Ok(x), Ok(y)) = (usize::try_from(x), usize::try_from(y)) else {
            return false;
        };
        x < self.width && y < self.height && self.bits[y * self.width + x]
    }

    /// Sets a pixel.
    pub fn set(&mut self, x: usize, y: usize, value: bool) {
        if x < self.width && y < self.height {
            self.bits[y * self.width + x] = value;
        }
    }

    /// Set pixels become clear and clear ones set.
    #[must_use]
    pub fn inverted(&self) -> Self {
        Self {
            width: self.width,
            height: self.height,
            bits: self.bits.iter().map(|b| !b).collect(),
        }
    }

    /// Number of set pixels.
    #[must_use]
    pub fn count(&self) -> usize {
        self.bits.iter().filter(|b| **b).count()
    }
}

/// Error correction of a QR code: how much of it may be damaged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QrEcc {
    /// About 7 %.
    Low,
    /// About 15 %.
    #[default]
    Medium,
    /// About 25 %.
    Quartile,
    /// About 30 %.
    High,
}

/// A point on the pixel grid, doubled so that half pixels are whole
/// numbers (x right, y down).
type Grid = (i64, i64);

/// Polygons (pixel grid, doubled coordinates, y down) of the set pixels:
/// one weakly simple polygon per region, holes joined by cuts.
#[must_use]
pub fn trace(bitmap: &Bitmap) -> Vec<Vec<Grid>> {
    let labels = regions(bitmap);
    // Edges with the set pixel on their right (seen with y down).
    let mut edges: Vec<(Grid, Grid, usize)> = Vec::new();
    for y in 0..i64::try_from(bitmap.height).unwrap_or(0) {
        for x in 0..i64::try_from(bitmap.width).unwrap_or(0) {
            if !bitmap.get(x, y) {
                continue;
            }
            let label = labels[index(bitmap, x, y)];
            let (x0, y0, x1, y1) = (2 * x, 2 * y, 2 * x + 2, 2 * y + 2);
            if !bitmap.get(x, y - 1) {
                edges.push(((x0, y0), (x1, y0), label));
            }
            if !bitmap.get(x + 1, y) {
                edges.push(((x1, y0), (x1, y1), label));
            }
            if !bitmap.get(x, y + 1) {
                edges.push(((x1, y1), (x0, y1), label));
            }
            if !bitmap.get(x - 1, y) {
                edges.push(((x0, y1), (x0, y0), label));
            }
        }
    }
    let mut starting: HashMap<Grid, Vec<usize>> = HashMap::new();
    for (i, (from, _, _)) in edges.iter().enumerate() {
        starting.entry(*from).or_default().push(i);
    }
    let mut used = vec![false; edges.len()];
    // Loops per region.
    let mut loops: HashMap<usize, Vec<Vec<Grid>>> = HashMap::new();
    for first in 0..edges.len() {
        if used[first] {
            continue;
        }
        let mut points = Vec::new();
        let mut edge = first;
        loop {
            used[edge] = true;
            let (from, to, _) = edges[edge];
            points.push(from);
            let direction = (to.0 - from.0, to.1 - from.1);
            // Turn right first: pixels that touch at a corner stay apart.
            let right = (-direction.1, direction.0);
            let candidates: Vec<usize> = starting
                .get(&to)
                .map(|list| list.iter().copied().filter(|e| !used[*e]).collect())
                .unwrap_or_default();
            let next = candidates
                .iter()
                .copied()
                .find(|e| {
                    let (a, b, _) = edges[*e];
                    (b.0 - a.0).signum() == right.0.signum()
                        && (b.1 - a.1).signum() == right.1.signum()
                })
                .or_else(|| candidates.first().copied());
            match next {
                Some(e) => edge = e,
                None => break,
            }
        }
        let label = edges[first].2;
        loops.entry(label).or_default().push(simplify(&points));
    }
    let mut keys: Vec<usize> = loops.keys().copied().collect();
    keys.sort_unstable();
    keys.into_iter()
        .filter_map(|label| {
            let mut list = loops.remove(&label)?;
            // The outline encloses the most; the others are holes.
            list.sort_by_key(|l| std::cmp::Reverse(area2(l)));
            let mut outline = list.remove(0);
            list.sort_by_key(|l| l.iter().map(|p| p.0).min().unwrap_or(0));
            for hole in list {
                outline = join(&outline, &hole);
            }
            Some(outline)
        })
        .collect()
}

fn index(bitmap: &Bitmap, x: i64, y: i64) -> usize {
    usize::try_from(y).unwrap_or(0) * bitmap.width + usize::try_from(x).unwrap_or(0)
}

/// A label per pixel: set pixels that touch at a side share one.
fn regions(bitmap: &Bitmap) -> Vec<usize> {
    let mut labels = vec![usize::MAX; bitmap.bits.len()];
    let mut next = 0;
    let mut stack = Vec::new();
    for start in 0..bitmap.bits.len() {
        if !bitmap.bits[start] || labels[start] != usize::MAX {
            continue;
        }
        labels[start] = next;
        stack.push(start);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % bitmap.width, i / bitmap.width);
            let mut visit = |j: usize| {
                if bitmap.bits[j] && labels[j] == usize::MAX {
                    labels[j] = next;
                    stack.push(j);
                }
            };
            if x > 0 {
                visit(i - 1);
            }
            if x + 1 < bitmap.width {
                visit(i + 1);
            }
            if y > 0 {
                visit(i - bitmap.width);
            }
            if y + 1 < bitmap.height {
                visit(i + bitmap.width);
            }
        }
        next += 1;
    }
    labels
}

/// Twice the signed area (positive for outlines, which run clockwise on
/// screen with y down).
fn area2(points: &[Grid]) -> i64 {
    let n = points.len();
    (0..n)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum()
}

/// Drops points in the middle of straight runs.
fn simplify(points: &[Grid]) -> Vec<Grid> {
    let n = points.len();
    (0..n)
        .filter(|&i| {
            let (a, b, c) = (points[(i + n - 1) % n], points[i], points[(i + 1) % n]);
            (b.0 - a.0) * (c.1 - b.1) != (b.1 - a.1) * (c.0 - b.0)
        })
        .map(|i| points[i])
        .collect()
}

/// Joins a hole to the polygon around it by a horizontal cut from the
/// hole's leftmost edge to the nearest edge on its left.
fn join(outline: &[Grid], hole: &[Grid]) -> Vec<Grid> {
    let corners = hole.len();
    let after = |index: usize| hole[(index + 1) % corners];
    // The leftmost vertical edge of the hole, its middle row.
    let Some(start) = (0..corners)
        .filter(|&index| hole[index].0 == after(index).0)
        .min_by_key(|&index| (hole[index].0, hole[index].1.min(after(index).1)))
    else {
        return outline.to_vec();
    };
    // An odd row (half a pixel) never meets a vertex.
    let row = hole[start].1.min(after(start).1) + 1;
    let inner = (hole[start].0, row);
    // The nearest vertical edge on the left that spans that row.
    let around = outline.len();
    let Some(edge) = (0..around)
        .filter(|&index| {
            let (from, to) = (outline[index], outline[(index + 1) % around]);
            from.0 == to.0 && from.0 < inner.0 && from.1.min(to.1) < row && row < from.1.max(to.1)
        })
        .max_by_key(|&index| outline[index].0)
    else {
        return outline.to_vec();
    };
    let outer = (outline[edge].0, row);
    let mut out = Vec::with_capacity(around + corners + 4);
    out.extend_from_slice(&outline[..=edge]);
    out.push(outer);
    out.push(inner);
    out.extend((1..=corners).map(|step| hole[(start + step) % corners]));
    out.push(inner);
    out.push(outer);
    out.extend_from_slice(&outline[edge + 1..]);
    out
}

/// The polygons of a picture on the board: `pixel` is the size of one
/// pixel, `center` the middle of the picture. Outlines run counter-clockwise
/// (the board's y axis points up).
#[must_use]
pub fn polygons(bitmap: &Bitmap, pixel: Nm, center: Point) -> Vec<Vec<Point>> {
    let half = pixel / 2;
    let width = i64::try_from(bitmap.width).unwrap_or(0);
    let height = i64::try_from(bitmap.height).unwrap_or(0);
    // Doubled grid units are half pixels.
    let left = center.x - width * half;
    let top = center.y + height * half;
    trace(bitmap)
        .into_iter()
        .map(|polygon| {
            // Clockwise on screen is clockwise on the board too: reversed,
            // the outlines run counter-clockwise.
            polygon
                .into_iter()
                .rev()
                .map(|(gx, gy)| Point::new(left + gx * half, top - gy * half))
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(rows: &[&str]) -> Bitmap {
        Bitmap::from_rows(rows).unwrap_or_else(|e| panic!("{e}"))
    }

    /// The area of the set pixels, from the polygons (in pixels).
    fn traced_area(bitmap: &Bitmap) -> i64 {
        trace(bitmap).iter().map(|p| area2(p)).sum::<i64>() / 8
    }

    #[test]
    fn a_square_is_four_corners() {
        let square = picture(&["##", "##"]);
        let polygons = trace(&square);
        assert_eq!(polygons, vec![vec![(0, 0), (4, 0), (4, 4), (0, 4)]]);
        assert_eq!(traced_area(&square), 4);
    }

    #[test]
    fn a_ring_gets_its_hole_by_a_cut() {
        let ring = picture(&["#####", "#...#", "#...#", "#####"]);
        let polygons = trace(&ring);
        assert_eq!(polygons.len(), 1, "one region: {polygons:?}");
        // Outline and hole: 4 + 4 corners, 2 points for each end of the cut.
        assert_eq!(polygons[0].len(), 12, "{:?}", polygons[0]);
        assert_eq!(traced_area(&ring), 20 - 6);
    }

    #[test]
    fn pixels_touching_at_a_corner_stay_apart() {
        let diagonal = picture(&["#.", ".#"]);
        assert_eq!(trace(&diagonal).len(), 2);
        assert_eq!(traced_area(&diagonal), 2);
        // A clear pixel touching the outside at a corner is no hole.
        let notch = picture(&["###", "##.", "###"]);
        assert_eq!(traced_area(&notch), 8);
    }

    #[test]
    fn several_holes_and_islands() {
        let letters = picture(&[
            "###########",
            "#.#.#.....#",
            "###.#.###.#",
            "....#.#.#.#",
            "....#.###.#",
            "....#.....#",
            "....#######",
        ]);
        let polygons = trace(&letters);
        // The box on the left and the frame (joined at the top, two holes),
        // and the island with its hole inside the frame.
        assert_eq!(polygons.len(), 2, "{polygons:?}");
        assert_eq!(
            traced_area(&letters),
            i64::try_from(letters.count()).unwrap_or(0)
        );
    }

    #[test]
    fn qr_codes_have_their_finder_patterns() {
        let qr = Bitmap::qr("https://example.com", QrEcc::Medium, 2);
        let qr = qr.unwrap_or_else(|e| panic!("{e}"));
        // Version 2: 25 modules, plus two on each side.
        assert_eq!((qr.width(), qr.height()), (29, 29));
        // The finder pattern at the top left: a 7 × 7 frame.
        assert!(qr.get(2, 2) && qr.get(8, 2) && qr.get(2, 8) && !qr.get(3, 3) && qr.get(4, 4));
        assert!(!qr.get(0, 0), "the quiet zone is clear");
        assert_eq!(traced_area(&qr), i64::try_from(qr.count()).unwrap_or(0));
        let long = "x".repeat(5000);
        assert_eq!(Bitmap::qr(&long, QrEcc::High, 0), Err(BitmapError::TooLong));
    }

    #[test]
    fn polygons_sit_around_the_center_with_y_up() {
        let bar = picture(&["##", ".."]);
        let polygons = polygons(&bar, 1_000_000, Point::new(0, 0));
        assert_eq!(
            polygons,
            vec![vec![
                Point::new(-1_000_000, 0),
                Point::new(1_000_000, 0),
                Point::new(1_000_000, 1_000_000),
                Point::new(-1_000_000, 1_000_000),
            ]]
        );
    }

    #[test]
    fn random_pictures_keep_their_area() {
        // A small linear congruential generator: the same pictures every run.
        let mut seed: u64 = 0x2545_f491_4f6c_dd1d;
        let mut random = move || {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            seed >> 33
        };
        for round in 0..300 {
            let size = 3 + usize::try_from(random() % 18).unwrap_or(0);
            let density = 30 + random() % 50;
            let mut bitmap = Bitmap::new(size, size);
            for y in 0..size {
                for x in 0..size {
                    bitmap.set(x, y, random() % 100 < density);
                }
            }
            let polygons = trace(&bitmap);
            assert_eq!(
                traced_area(&bitmap),
                i64::try_from(bitmap.count()).unwrap_or(0),
                "round {round}"
            );
            assert!(polygons.iter().all(|p| p.len() >= 4), "round {round}");
        }
    }

    #[test]
    fn rows_are_checked() {
        assert_eq!(Bitmap::from_rows(&["##", "#"]), Err(BitmapError::Ragged));
        assert_eq!(Bitmap::from_rows::<&str>(&[]), Err(BitmapError::Ragged));
        assert_eq!(Bitmap::from_rows(&["#a"]), Err(BitmapError::Character('a')));
    }
}
