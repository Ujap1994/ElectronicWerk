//! Stable object identifiers.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A stable, globally unique object identifier (UUID).
///
/// Serialized as a lowercase hyphenated string. Ordering follows the string
/// order, which keeps saved collections deterministic.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Id(Uuid);

impl Id {
    /// The all-zero id. Never assigned to a real object.
    pub const NIL: Self = Self(Uuid::nil());

    /// A new random (version 4) id.
    #[must_use]
    pub fn random() -> Self {
        Self(Uuid::new_v4())
    }

    /// A version 4 id built from caller-supplied random bytes (used by
    /// deterministic generators).
    #[must_use]
    pub fn from_random_bytes(bytes: [u8; 16]) -> Self {
        Self(uuid::Builder::from_random_bytes(bytes).into_uuid())
    }

    /// An id from its 128-bit value.
    #[must_use]
    pub const fn from_u128(value: u128) -> Self {
        Self(Uuid::from_u128(value))
    }

    /// The 128-bit value.
    #[must_use]
    pub const fn as_u128(self) -> u128 {
        self.0.as_u128()
    }

    /// `true` for [`Id::NIL`].
    #[must_use]
    pub const fn is_nil(self) -> bool {
        self.0.is_nil()
    }

    /// A stable id derived from text (FNV-1a, formatted as a UUID version
    /// 8): the same parts always give the same id.
    #[must_use]
    pub fn derived(parts: &[&str]) -> Self {
        let mut a: u64 = 0xcbf2_9ce4_8422_2325;
        let mut b: u64 = 0x8422_2325_cbf2_9ce4;
        for part in parts {
            for byte in part.bytes().chain(std::iter::once(0)) {
                a = (a ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3);
                b = (b ^ u64::from(byte.rotate_left(3))).wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        let mut value = (u128::from(a) << 64) | u128::from(b);
        // Version 8 (custom) and the RFC 4122 variant.
        value = (value & !(0xF_u128 << 76)) | (0x8_u128 << 76);
        value = (value & !(0x3_u128 << 62)) | (0x2_u128 << 62);
        Self::from_u128(value)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0.hyphenated(), f)
    }
}

impl fmt::Debug for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Error for a string that is not a valid id.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a valid id (expected a UUID)")]
pub struct ParseIdError(String);

impl FromStr for Id {
    type Err = ParseIdError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(text)
            .map(Self)
            .map_err(|_| ParseIdError(text.chars().take(64).collect()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_round_trip() {
        let a = Id::random();
        let b = Id::random();
        assert_ne!(a, b);
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json.len(), 38);
        assert_eq!(serde_json::from_str::<Id>(&json).unwrap(), a);
        assert_eq!(a.to_string().parse::<Id>().unwrap(), a);
        assert_eq!(format!("{a:?}"), a.to_string());
    }

    #[test]
    fn deterministic_ids_are_version_4() {
        let id = Id::from_random_bytes([7; 16]);
        assert_eq!(id, Id::from_random_bytes([7; 16]));
        assert_eq!(id.to_string().as_bytes()[14], b'4');
        assert_eq!(Id::from_u128(id.as_u128()), id);
    }

    #[test]
    fn order_matches_string_order() {
        let mut ids: Vec<Id> = (0..50).map(|_| Id::random()).collect();
        ids.sort();
        let strings: Vec<String> = ids.iter().map(ToString::to_string).collect();
        let mut sorted = strings.clone();
        sorted.sort();
        assert_eq!(strings, sorted);
    }

    #[test]
    fn rejects_garbage() {
        assert!("nope".parse::<Id>().is_err());
        assert!(serde_json::from_str::<Id>("42").is_err());
        assert!(Id::NIL.is_nil());
    }
}
