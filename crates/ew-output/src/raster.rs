//! Rasterizes images for comparisons: generated files are checked against
//! reference files by drawing both into bitmaps and counting the pixels
//! that differ.

use ew_pcb::geom::{Core, Shape, V};

use crate::read::Image;

/// Chord tolerance of curves in rasterized shapes (nm).
const TOLERANCE: f64 = 500.0;

/// A black-and-white bitmap over a board area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    /// Lower-left corner (nm).
    pub min: [i64; 2],
    /// Pixel size (nm).
    pub pixel: i64,
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Pixels, row by row from the bottom.
    pub bits: Vec<bool>,
}

impl Bitmap {
    /// An empty bitmap covering `[min x, min y, max x, max y]`.
    #[must_use]
    pub fn new(bounds: [f64; 4], pixel: f64) -> Self {
        let width = ((bounds[2] - bounds[0]) / pixel).ceil().max(1.0) as usize;
        let height = ((bounds[3] - bounds[1]) / pixel).ceil().max(1.0) as usize;
        Self {
            min: [bounds[0].floor() as i64, bounds[1].floor() as i64],
            pixel: pixel as i64,
            width,
            height,
            bits: vec![false; width * height],
        }
    }

    /// Draws an image (dark sets, clear resets, in order), moved by
    /// `offset`.
    #[must_use]
    pub fn of(image: &Image, bounds: [f64; 4], pixel: f64, offset: V) -> Self {
        let mut bitmap = Self::new(bounds, pixel);
        for object in &image.objects {
            bitmap.paint(&object.shape, object.dark, offset);
        }
        bitmap
    }

    fn center(&self, column: usize, row: usize) -> V {
        let p = self.pixel as f64;
        V::new(
            self.min[0] as f64 + (column as f64 + 0.5) * p,
            self.min[1] as f64 + (row as f64 + 0.5) * p,
        )
    }

    /// The pixel range covered by a box.
    fn range(&self, lo: V, hi: V) -> Option<(usize, usize, usize, usize)> {
        let p = self.pixel as f64;
        let c0 = ((lo.x - self.min[0] as f64) / p - 0.5).ceil().max(0.0) as usize;
        let r0 = ((lo.y - self.min[1] as f64) / p - 0.5).ceil().max(0.0) as usize;
        let c1 = ((hi.x - self.min[0] as f64) / p - 0.5).floor();
        let r1 = ((hi.y - self.min[1] as f64) / p - 0.5).floor();
        if c1 < 0.0 || r1 < 0.0 {
            return None;
        }
        let c1 = (c1 as usize).min(self.width.saturating_sub(1));
        let r1 = (r1 as usize).min(self.height.saturating_sub(1));
        (c0 <= c1 && r0 <= r1).then_some((c0, r0, c1, r1))
    }

    /// Sets (or clears) the pixels whose centres lie in the shape.
    pub fn paint(&mut self, shape: &Shape, value: bool, offset: V) {
        if let Core::Poly(poly) = &shape.core {
            let polygons = if shape.radius.abs() < 0.5 {
                vec![poly.rings().to_vec()]
            } else {
                shape.outlines(0.0, TOLERANCE)
            };
            for rings in polygons {
                let rings: Vec<Vec<V>> = rings
                    .into_iter()
                    .map(|ring| ring.into_iter().map(|p| p.add(offset)).collect())
                    .collect();
                self.fill(&rings, value);
            }
            return;
        }
        let b = shape.bbox();
        let Some((c0, r0, c1, r1)) = self.range(
            V::new(b.min.x + offset.x, b.min.y + offset.y),
            V::new(b.max.x + offset.x, b.max.y + offset.y),
        ) else {
            return;
        };
        for row in r0..=r1 {
            for column in c0..=c1 {
                let p = self.center(column, row).sub(offset);
                if inside(shape, p) {
                    self.bits[row * self.width + column] = value;
                }
            }
        }
    }

    /// Fills rings with the non-zero rule.
    fn fill(&mut self, rings: &[Vec<V>], value: bool) {
        let mut lo = V::new(f64::MAX, f64::MAX);
        let mut hi = V::new(f64::MIN, f64::MIN);
        for p in rings.iter().flatten() {
            lo = V::new(lo.x.min(p.x), lo.y.min(p.y));
            hi = V::new(hi.x.max(p.x), hi.y.max(p.y));
        }
        let Some((c0, r0, c1, r1)) = self.range(lo, hi) else {
            return;
        };
        let mut crossings: Vec<(f64, i32)> = Vec::new();
        for row in r0..=r1 {
            let y = self.center(0, row).y;
            crossings.clear();
            for ring in rings {
                let n = ring.len();
                for i in 0..n {
                    let (a, b) = (ring[i], ring[(i + 1) % n]);
                    if (a.y <= y) != (b.y <= y) {
                        let t = (y - a.y) / (b.y - a.y);
                        crossings.push((a.x + t * (b.x - a.x), if b.y > a.y { 1 } else { -1 }));
                    }
                }
            }
            crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut winding = 0;
            for pair in crossings.windows(2) {
                winding += pair[0].1;
                if winding != 0 {
                    let from = pair[0].0;
                    let to = pair[1].0;
                    let Some((s0, _, s1, _)) = self.range(V::new(from, y), V::new(to, y)) else {
                        continue;
                    };
                    for column in s0.max(c0)..=s1.min(c1) {
                        self.bits[row * self.width + column] = value;
                    }
                }
            }
        }
    }

    /// Number of set pixels.
    #[must_use]
    pub fn count(&self) -> usize {
        self.bits.iter().filter(|b| **b).count()
    }

    /// Number of pixels set in exactly one of two bitmaps of the same area.
    #[must_use]
    pub fn difference(&self, other: &Self) -> usize {
        self.bits
            .iter()
            .zip(&other.bits)
            .filter(|(a, b)| a != b)
            .count()
    }

    /// The bitmap as a portable graymap (black on white), for looking at
    /// differences.
    #[must_use]
    pub fn pgm(&self) -> Vec<u8> {
        let mut out = format!("P5 {} {} 255\n", self.width, self.height).into_bytes();
        for row in (0..self.height).rev() {
            for column in 0..self.width {
                out.push(if self.bits[row * self.width + column] {
                    0
                } else {
                    255
                });
            }
        }
        out
    }
}

/// `true` if the point lies in the shape (discs and stadiums).
fn inside(shape: &Shape, p: V) -> bool {
    match &shape.core {
        Core::Point(c) => c.dist(p) <= shape.radius,
        Core::Segment(a, b) => ew_pcb::geom::point_segment(p, *a, *b) <= shape.radius,
        Core::Poly(_) => shape.contains(p),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::Object;

    #[test]
    fn paints_and_clears() {
        let image = Image {
            function: None,
            objects: vec![
                Object {
                    dark: true,
                    shape: Shape::polygon(
                        vec![
                            V::new(0.0, 0.0),
                            V::new(100.0, 0.0),
                            V::new(100.0, 100.0),
                            V::new(0.0, 100.0),
                        ],
                        Vec::new(),
                        0.0,
                    ),
                },
                Object {
                    dark: false,
                    shape: Shape::disc(V::new(50.0, 50.0), 20.0),
                },
            ],
            warnings: Vec::new(),
        };
        let bitmap = Bitmap::of(&image, [0.0, 0.0, 100.0, 100.0], 1.0, V::new(0.0, 0.0));
        let expected = 10_000.0 - std::f64::consts::PI * 400.0;
        assert!(
            (bitmap.count() as f64 - expected).abs() < 40.0,
            "{}",
            bitmap.count()
        );
        let empty = Bitmap::new([0.0, 0.0, 100.0, 100.0], 1.0);
        assert_eq!(bitmap.difference(&empty), bitmap.count());
        assert!(bitmap.pgm().starts_with(b"P5 100 100 255\n"));
    }
}
