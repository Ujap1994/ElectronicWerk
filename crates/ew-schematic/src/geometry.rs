//! Placement geometry of schematic objects, in exact integer nanometres
//! wherever the angles allow it.

use ew_model::library::{Pin, PinDirection};
use ew_model::schematic::{PlacedSymbol, Schematic, SheetPin, SheetSide, SheetSymbol};
use ew_model::units::{Angle, Nm, Point};

/// Turns a point counterclockwise about the origin (exact for multiples of
/// 90°).
#[must_use]
pub fn rotate(p: Point, angle: Angle) -> Point {
    match angle.millidegrees().rem_euclid(360_000) {
        0 => p,
        90_000 => Point::new(-p.y, p.x),
        180_000 => Point::new(-p.x, -p.y),
        270_000 => Point::new(p.y, -p.x),
        _ => {
            let (sin, cos) = angle.degrees().to_radians().sin_cos();
            let (x, y) = (p.x as f64, p.y as f64);
            Point::new(
                (x * cos - y * sin).round() as Nm,
                (x * sin + y * cos).round() as Nm,
            )
        }
    }
}

/// Maps a point of a symbol definition to the sheet: mirror x, then rotate,
/// then move to the symbol's position (the same order as the 2D view).
#[must_use]
pub fn place(symbol: &PlacedSymbol, local: Point) -> Point {
    let mirrored = if symbol.mirror {
        Point::new(-local.x, local.y)
    } else {
        local
    };
    let turned = rotate(mirrored, symbol.rotation);
    Point::new(symbol.at.x + turned.x, symbol.at.y + turned.y)
}

/// The connection point of a pin of a placed symbol.
#[must_use]
pub fn pin_point(symbol: &PlacedSymbol, pin: &Pin) -> Point {
    place(symbol, pin.at)
}

/// The direction a pin points to from its connection point (towards the
/// symbol body), on the sheet: a unit vector in steps of 90°.
#[must_use]
pub fn pin_direction(symbol: &PlacedSymbol, pin: &Pin) -> Point {
    let local = match pin.direction {
        PinDirection::Right => Point::new(1, 0),
        PinDirection::Left => Point::new(-1, 0),
        PinDirection::Up => Point::new(0, 1),
        PinDirection::Down => Point::new(0, -1),
    };
    let mirrored = if symbol.mirror {
        Point::new(-local.x, local.y)
    } else {
        local
    };
    rotate(mirrored, symbol.rotation)
}

/// The connection point of a sheet pin (sheets are placed by their top-left
/// corner and extend down, y up).
#[must_use]
pub fn sheet_pin_point(sheet: &SheetSymbol, pin: &SheetPin) -> Point {
    let Point { x, y } = sheet.at;
    match pin.side {
        SheetSide::Left => Point::new(x, y - pin.offset),
        SheetSide::Right => Point::new(x + sheet.size.w, y - pin.offset),
        SheetSide::Top => Point::new(x + pin.offset, y),
        SheetSide::Bottom => Point::new(x + pin.offset, y - sheet.size.h),
    }
}

/// `true` if `p` lies on the segment `a`–`b` (exact).
#[must_use]
pub fn on_segment(p: Point, a: Point, b: Point) -> bool {
    let cross = i128::from(b.x - a.x) * i128::from(p.y - a.y)
        - i128::from(b.y - a.y) * i128::from(p.x - a.x);
    cross == 0
        && p.x >= a.x.min(b.x)
        && p.x <= a.x.max(b.x)
        && p.y >= a.y.min(b.y)
        && p.y <= a.y.max(b.y)
}

/// `true` if `p` lies strictly inside the segment `a`–`b` (not on an end).
#[must_use]
pub fn inside_segment(p: Point, a: Point, b: Point) -> bool {
    p != a && p != b && on_segment(p, a, b)
}

/// The grid origin of a sheet: its top-left page corner (as in KiCad, so
/// imported drawings stay on the grid).
#[must_use]
pub fn grid_origin(sheet: &Schematic) -> Point {
    Point::new(0, sheet.page.dimensions().h)
}

/// Rounds a point to the nearest point of a grid through `origin`.
#[must_use]
pub fn snap(p: Point, grid: Nm, origin: Point) -> Point {
    if grid <= 0 {
        return p;
    }
    let round = |v: Nm, o: Nm| ((v - o) as f64 / grid as f64).round() as Nm * grid + o;
    Point::new(round(p.x, origin.x), round(p.y, origin.y))
}

/// `true` if a point is on the grid through `origin`.
#[must_use]
pub const fn on_grid(p: Point, grid: Nm, origin: Point) -> bool {
    grid <= 0 || ((p.x - origin.x) % grid == 0 && (p.y - origin.y) % grid == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ew_model::units::mm;

    #[test]
    fn rotations_are_exact_for_quarter_turns() {
        let p = Point::new(mm(3), mm(1));
        assert_eq!(rotate(p, Angle::DEG_90), Point::new(-mm(1), mm(3)));
        assert_eq!(
            rotate(p, Angle::from_degrees(180)),
            Point::new(-mm(3), -mm(1))
        );
        assert_eq!(
            rotate(p, Angle::from_degrees(270)),
            Point::new(mm(1), -mm(3))
        );
        assert_eq!(
            rotate(p, Angle::from_degrees(-90)),
            Point::new(mm(1), -mm(3))
        );
    }

    #[test]
    fn points_on_segments() {
        let (a, b) = (Point::new(0, 0), Point::new(mm(10), 0));
        assert!(on_segment(Point::new(mm(5), 0), a, b));
        assert!(on_segment(a, a, b));
        assert!(!inside_segment(a, a, b));
        assert!(inside_segment(Point::new(mm(5), 0), a, b));
        assert!(!on_segment(Point::new(mm(11), 0), a, b));
        assert!(!on_segment(Point::new(mm(5), 1), a, b));
        assert!(on_segment(
            Point::new(mm(2), mm(2)),
            a,
            Point::new(mm(4), mm(4))
        ));
        assert_eq!(
            snap(Point::new(1_300_000, -1_200_000), 1_270_000, Point::ORIGIN),
            Point::new(1_270_000, -1_270_000)
        );
        let origin = Point::new(0, mm(210));
        assert_eq!(
            snap(Point::new(1_300_000, mm(209)), 1_270_000, origin),
            Point::new(1_270_000, mm(210) - 1_270_000)
        );
        assert!(on_grid(
            Point::new(1_270_000, mm(210) - 2_540_000),
            1_270_000,
            origin
        ));
        assert!(!on_grid(Point::new(1_270_000, mm(100)), 1_270_000, origin));
    }
}
