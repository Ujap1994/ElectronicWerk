//! Units and basic geometry values.
//!
//! All lengths are integer nanometres. Integer coordinates have no rounding
//! errors, compare exactly and serialize deterministically.

use std::fmt;
use std::ops::{Add, Neg, Sub};

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A length or coordinate in nanometres.
pub type Nm = i64;

/// Nanometres per micrometre.
pub const NM_PER_UM: Nm = 1_000;
/// Nanometres per millimetre.
pub const NM_PER_MM: Nm = 1_000_000;
/// Nanometres per mil (1/1000 inch).
pub const NM_PER_MIL: Nm = 25_400;
/// Nanometres per inch.
pub const NM_PER_INCH: Nm = 25_400_000;

/// Largest absolute coordinate the model accepts (10 m).
pub const MAX_COORD: Nm = 10_000_000_000;

/// Whole millimetres to nanometres.
#[must_use]
pub const fn mm(value: i64) -> Nm {
    value * NM_PER_MM
}

/// Micrometres to nanometres.
#[must_use]
pub const fn um(value: i64) -> Nm {
    value * NM_PER_UM
}

/// Mils (1/1000 inch) to nanometres.
#[must_use]
pub const fn mil(value: i64) -> Nm {
    value * NM_PER_MIL
}

/// Millimetres (with fractions) to nanometres, rounded to the nearest
/// nanometre. Non-finite input yields 0.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    reason = "the value is rounded and clamped to the coordinate range first"
)]
pub fn from_mm(value: f64) -> Nm {
    if !value.is_finite() {
        return 0;
    }
    let limit = 1e13;
    (value * 1e6).round().clamp(-limit, limit) as Nm
}

/// Nanometres to millimetres.
#[must_use]
#[allow(
    clippy::cast_precision_loss,
    reason = "board coordinates stay far below 2^52 nm"
)]
pub fn to_mm(value: Nm) -> f64 {
    value as f64 / 1e6
}

/// A point (or vector) in nanometres. Serialized as `[x, y]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Point {
    /// Horizontal coordinate (positive to the right).
    pub x: Nm,
    /// Vertical coordinate (positive upwards).
    pub y: Nm,
}

impl Point {
    /// The origin `(0, 0)`.
    pub const ORIGIN: Self = Self { x: 0, y: 0 };

    /// Creates a point.
    #[must_use]
    pub const fn new(x: Nm, y: Nm) -> Self {
        Self { x, y }
    }

    /// Point in whole millimetres.
    #[must_use]
    pub const fn mm(x: i64, y: i64) -> Self {
        Self::new(mm(x), mm(y))
    }

    /// `true` if both coordinates lie within ±[`MAX_COORD`].
    #[must_use]
    pub const fn in_range(self) -> bool {
        self.x.abs() <= MAX_COORD && self.y.abs() <= MAX_COORD
    }
}

impl Add for Point {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for Point {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }
}

impl Neg for Point {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.x, -self.y)
    }
}

impl Serialize for Point {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        [self.x, self.y].serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Point {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let [x, y] = <[Nm; 2]>::deserialize(deserializer)?;
        Ok(Self { x, y })
    }
}

/// A width and height in nanometres. Serialized as `[w, h]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Size {
    /// Width (x extent).
    pub w: Nm,
    /// Height (y extent).
    pub h: Nm,
}

impl Size {
    /// Creates a size.
    #[must_use]
    pub const fn new(w: Nm, h: Nm) -> Self {
        Self { w, h }
    }

    /// Size in whole millimetres.
    #[must_use]
    pub const fn mm(w: i64, h: i64) -> Self {
        Self::new(mm(w), mm(h))
    }
}

impl Serialize for Size {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        [self.w, self.h].serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Size {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let [w, h] = <[Nm; 2]>::deserialize(deserializer)?;
        Ok(Self { w, h })
    }
}

/// A 3D vector in nanometres (3D model offsets). Serialized as `[x, y, z]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Vec3 {
    /// X component.
    pub x: Nm,
    /// Y component.
    pub y: Nm,
    /// Z component (positive away from the board top).
    pub z: Nm,
}

impl Serialize for Vec3 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        [self.x, self.y, self.z].serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Vec3 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let [x, y, z] = <[Nm; 3]>::deserialize(deserializer)?;
        Ok(Self { x, y, z })
    }
}

/// An axis-aligned rectangle (bounding box).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rect {
    /// Lower-left corner.
    pub min: Point,
    /// Upper-right corner.
    pub max: Point,
}

impl Rect {
    /// A rectangle spanning two corners in any order.
    #[must_use]
    pub fn from_corners(a: Point, b: Point) -> Self {
        Self {
            min: Point::new(a.x.min(b.x), a.y.min(b.y)),
            max: Point::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    /// Bounding box of a set of points, or `None` if there are none.
    #[must_use]
    pub fn from_points(points: impl IntoIterator<Item = Point>) -> Option<Self> {
        let mut iter = points.into_iter();
        let first = iter.next()?;
        let mut rect = Self {
            min: first,
            max: first,
        };
        for point in iter {
            rect.include(point);
        }
        Some(rect)
    }

    /// Grows the rectangle to contain `point`.
    pub fn include(&mut self, point: Point) {
        self.min.x = self.min.x.min(point.x);
        self.min.y = self.min.y.min(point.y);
        self.max.x = self.max.x.max(point.x);
        self.max.y = self.max.y.max(point.y);
    }

    /// Smallest rectangle containing both.
    #[must_use]
    pub fn union(self, other: Self) -> Self {
        let mut rect = self;
        rect.include(other.min);
        rect.include(other.max);
        rect
    }

    /// Width in nanometres.
    #[must_use]
    pub const fn width(self) -> Nm {
        self.max.x - self.min.x
    }

    /// Height in nanometres.
    #[must_use]
    pub const fn height(self) -> Nm {
        self.max.y - self.min.y
    }
}

/// An angle in thousandths of a degree, counter-clockwise, normalized to
/// `[0°, 360°)`.
///
/// Serialized as degrees: a whole number when possible (`90`), otherwise
/// with up to three decimals (`22.5`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Angle(i32);

impl Angle {
    /// 0°.
    pub const ZERO: Self = Self(0);
    /// 90°.
    pub const DEG_90: Self = Self(90_000);
    /// 180°.
    pub const DEG_180: Self = Self(180_000);
    /// 270°.
    pub const DEG_270: Self = Self(270_000);

    const FULL: i64 = 360_000;

    /// Angle from thousandths of a degree (any value, normalized).
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "rem_euclid keeps the value in 0..360_000"
    )]
    pub const fn from_millidegrees(value: i64) -> Self {
        Self(value.rem_euclid(Self::FULL) as i32)
    }

    /// Angle from whole degrees (any value, normalized).
    #[must_use]
    pub const fn from_degrees(value: i64) -> Self {
        Self::from_millidegrees((value % 360) * 1_000)
    }

    /// Angle from fractional degrees, rounded to 0.001°. Returns `None` for
    /// non-finite input.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the value is reduced modulo 360 before the cast"
    )]
    pub fn from_degrees_f64(value: f64) -> Option<Self> {
        if !value.is_finite() {
            return None;
        }
        let reduced = (value % 360.0) * 1_000.0;
        Some(Self::from_millidegrees(reduced.round() as i64))
    }

    /// Thousandths of a degree in `0..360_000`.
    #[must_use]
    pub const fn millidegrees(self) -> i32 {
        self.0
    }

    /// Degrees as a float.
    #[must_use]
    pub fn degrees(self) -> f64 {
        f64::from(self.0) / 1_000.0
    }

    /// `true` for multiples of 90°.
    #[must_use]
    pub const fn is_orthogonal(self) -> bool {
        self.0 % 90_000 == 0
    }
}

impl fmt::Display for Angle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 % 1_000 == 0 {
            write!(f, "{}°", self.0 / 1_000)
        } else {
            write!(f, "{}°", self.degrees())
        }
    }
}

impl Serialize for Angle {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0 % 1_000 == 0 {
            serializer.serialize_i32(self.0 / 1_000)
        } else {
            serializer.serialize_f64(self.degrees())
        }
    }
}

impl<'de> Deserialize<'de> for Angle {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct AngleVisitor;

        impl Visitor<'_> for AngleVisitor {
            type Value = Angle;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an angle in degrees")
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Angle, E> {
                Ok(Angle::from_degrees(value))
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Angle, E> {
                Ok(Angle::from_degrees(i64::try_from(value % 360).unwrap_or(0)))
            }

            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Angle, E> {
                Angle::from_degrees_f64(value)
                    .ok_or_else(|| E::custom("angle must be a finite number"))
            }
        }

        deserializer.deserialize_any(AngleVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn points_serialize_as_pairs() {
        let json = serde_json::to_string(&Point::new(1, -2)).unwrap();
        assert_eq!(json, "[1,-2]");
        assert_eq!(
            serde_json::from_str::<Point>("[5,6]").unwrap(),
            Point::new(5, 6)
        );
        assert!(serde_json::from_str::<Point>("[1]").is_err());
        assert_eq!(
            serde_json::to_string(&Size::mm(1, 2)).unwrap(),
            "[1000000,2000000]"
        );
    }

    #[test]
    fn unit_conversions() {
        assert_eq!(mm(1), 1_000_000);
        assert_eq!(um(250), 250_000);
        assert_eq!(mil(10), 254_000);
        assert_eq!(from_mm(0.2), 200_000);
        assert_eq!(from_mm(-1.000_000_4), -1_000_000);
        assert_eq!(from_mm(f64::NAN), 0);
        assert!((to_mm(1_500_000) - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn angles_normalize_and_serialize_as_degrees() {
        assert_eq!(Angle::from_degrees(-90), Angle::DEG_270);
        assert_eq!(Angle::from_degrees(450), Angle::DEG_90);
        assert_eq!(serde_json::to_string(&Angle::DEG_90).unwrap(), "90");
        assert_eq!(
            serde_json::to_string(&Angle::from_millidegrees(22_500)).unwrap(),
            "22.5"
        );
        assert_eq!(
            serde_json::from_str::<Angle>("-45").unwrap(),
            Angle::from_degrees(315)
        );
        assert_eq!(
            serde_json::from_str::<Angle>("12.3456")
                .unwrap()
                .millidegrees(),
            12_346
        );
        assert!(serde_json::from_str::<Angle>("\"x\"").is_err());
        assert_eq!(Angle::from_millidegrees(22_500).to_string(), "22.5°");
        assert!(Angle::DEG_180.is_orthogonal());
    }

    #[test]
    fn rect_bounds() {
        let rect = Rect::from_points([Point::mm(3, -1), Point::mm(-2, 4)]).unwrap();
        assert_eq!(rect.width(), mm(5));
        assert_eq!(rect.height(), mm(5));
        assert!(Rect::from_points([]).is_none());
        let union = rect.union(Rect::from_corners(Point::mm(10, 10), Point::mm(11, 11)));
        assert_eq!(union.max, Point::mm(11, 11));
    }

    proptest! {
        #[test]
        fn angle_round_trips(millideg in -10_000_000_i64..10_000_000) {
            let angle = Angle::from_millidegrees(millideg);
            let json = serde_json::to_string(&angle).unwrap();
            prop_assert_eq!(serde_json::from_str::<Angle>(&json).unwrap(), angle);
            prop_assert!((0..360_000).contains(&angle.millidegrees()));
        }

        #[test]
        fn point_round_trips(x in any::<i64>(), y in any::<i64>()) {
            let point = Point::new(x, y);
            let json = serde_json::to_string(&point).unwrap();
            prop_assert_eq!(serde_json::from_str::<Point>(&json).unwrap(), point);
        }
    }
}
