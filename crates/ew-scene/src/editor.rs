//! Scenes of the symbol and footprint editors. Every pin, pad, graphic and
//! text is an object of its own; its id encodes the kind (high 64 bits)
//! and the index in the list of that kind (low 64 bits), so the editor can
//! address items that have no ids in the data model.

use ew_model::id::Id;

/// Pin of a symbol.
pub const PIN: u64 = 1;
/// Graphic of a symbol or footprint.
pub const GRAPHIC: u64 = 2;
/// Text of a symbol or footprint.
pub const TEXT: u64 = 3;
/// Pad of a footprint.
pub const PAD: u64 = 4;

/// The id of an item.
#[must_use]
pub const fn item_id(tag: u64, index: usize) -> Id {
    Id::from_u128(((tag as u128) << 64) | index as u128)
}

/// The kind tag and index of an item id.
#[must_use]
pub const fn decode(id: Id) -> (u64, usize) {
    let value = id.as_u128();
    (
        (value >> 64) as u64,
        (value & 0xFFFF_FFFF_FFFF_FFFF) as usize,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip() {
        assert_eq!(decode(item_id(PAD, 41)), (PAD, 41));
        assert_eq!(
            item_id(PIN, 3).to_string(),
            "00000000-0000-0001-0000-000000000003"
        );
    }
}
