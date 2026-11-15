//! Gerber and Excellon files as a reference drawing on a board: the dark
//! areas of a file (clear areas removed) as filled single-contour
//! polygons, for tracing a board from its manufacturing files or comparing
//! a design with them.

use ew_pcb::geom::V;
use ew_pcb::poly::{Polygons, difference, union};

use crate::fracture::fracture;
use crate::read::Image;

/// Curves become segments within this deviation (nm).
pub const TOLERANCE: f64 = 2_000.0;

/// The areas of an image in file coordinates (y up): every dark shape
/// united, clear shapes removed in drawing order, each area one contour
/// (holes joined by cut-ins).
#[must_use]
pub fn areas(image: &Image) -> Vec<Vec<V>> {
    let mut result: Polygons = Vec::new();
    let mut run: Polygons = Vec::new();
    let mut dark = true;
    let flush = |result: &mut Polygons, run: &mut Polygons, dark: bool| {
        if run.is_empty() {
            return;
        }
        *result = if dark {
            let mut all = std::mem::take(result);
            all.append(run);
            union(&all)
        } else {
            difference(result, run)
        };
        run.clear();
    };
    for object in &image.objects {
        if object.dark != dark {
            flush(&mut result, &mut run, dark);
            dark = object.dark;
        }
        run.extend(object.shape.outlines(0.0, TOLERANCE));
    }
    flush(&mut result, &mut run, dark);
    result
        .iter()
        .filter_map(|polygon| {
            let (outline, holes) = polygon.split_first()?;
            (outline.len() >= 3).then(|| fracture(outline, holes))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read;

    #[test]
    fn dark_areas_unite_and_clear_areas_cut_holes() {
        // Two overlapping 2 × 2 mm squares, then a clear 0.5 mm square in
        // the first one.
        let text = "%FSLAX46Y46*%\n%MOMM*%\n%ADD10C,0.1*%\n\
            G36*X0Y0D02*X2000000Y0D01*X2000000Y2000000D01*X0Y2000000D01*X0Y0D01*G37*\n\
            G36*X1000000Y1000000D02*X3000000Y1000000D01*X3000000Y3000000D01*X1000000Y3000000D01*X1000000Y1000000D01*G37*\n\
            %LPC*%\n\
            G36*X250000Y250000D02*X750000Y250000D01*X750000Y750000D01*X250000Y750000D01*X250000Y250000D01*G37*\n\
            M02*\n";
        let image = read::gerber(text);
        let areas = areas(&image);
        assert_eq!(areas.len(), 1);
        // 4 + 4 - 1 (overlap) - 0.25 (hole) mm².
        let area = ew_pcb::geom::signed_area2(&areas[0]).abs() / 2.0 / 1e12;
        assert!((area - 6.75).abs() < 1e-3, "{area}");
    }
}
