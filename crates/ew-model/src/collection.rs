//! Id-keyed object collections.

use std::collections::BTreeMap;
use std::collections::btree_map;
use std::fmt;
use std::marker::PhantomData;

use serde::de::{self, SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::id::Id;

/// An object with a stable id.
pub trait Keyed {
    /// The object's id.
    fn id(&self) -> Id;
}

/// Objects of one kind, keyed and ordered by id.
///
/// Serialized as a JSON array sorted by id, so a file does not change when
/// objects are merely created in a different order. Duplicate ids are
/// rejected when reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collection<T>(BTreeMap<Id, T>);

impl<T> Default for Collection<T> {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}

impl<T: Keyed> Collection<T> {
    /// An empty collection.
    #[must_use]
    pub const fn new() -> Self {
        Self(BTreeMap::new())
    }

    /// Inserts or replaces an object and returns the previous one.
    pub fn insert(&mut self, item: T) -> Option<T> {
        self.0.insert(item.id(), item)
    }

    /// Removes an object.
    pub fn remove(&mut self, id: Id) -> Option<T> {
        self.0.remove(&id)
    }

    /// The object with this id.
    #[must_use]
    pub fn get(&self, id: Id) -> Option<&T> {
        self.0.get(&id)
    }

    /// Mutable access to the object with this id.
    pub fn get_mut(&mut self, id: Id) -> Option<&mut T> {
        self.0.get_mut(&id)
    }

    /// `true` if an object with this id exists.
    #[must_use]
    pub fn contains(&self, id: Id) -> bool {
        self.0.contains_key(&id)
    }

    /// Number of objects.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `true` if there are no objects.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Objects in id order.
    pub fn iter(&self) -> btree_map::Values<'_, Id, T> {
        self.0.values()
    }

    /// Mutable objects in id order.
    pub fn iter_mut(&mut self) -> btree_map::ValuesMut<'_, Id, T> {
        self.0.values_mut()
    }

    /// Ids in order.
    pub fn ids(&self) -> impl Iterator<Item = Id> + '_ {
        self.0.keys().copied()
    }
}

impl<T: Keyed> FromIterator<T> for Collection<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut collection = Self::new();
        for item in iter {
            collection.insert(item);
        }
        collection
    }
}

impl<'a, T> IntoIterator for &'a Collection<T> {
    type Item = &'a T;
    type IntoIter = btree_map::Values<'a, Id, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.values()
    }
}

impl<'a, T> IntoIterator for &'a mut Collection<T> {
    type Item = &'a mut T;
    type IntoIter = btree_map::ValuesMut<'a, Id, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.values_mut()
    }
}

impl<T: Serialize> Serialize for Collection<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for item in self.0.values() {
            seq.serialize_element(item)?;
        }
        seq.end()
    }
}

impl<'de, T: Deserialize<'de> + Keyed> Deserialize<'de> for Collection<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SeqVisitor<T>(PhantomData<T>);

        impl<'de, T: Deserialize<'de> + Keyed> Visitor<'de> for SeqVisitor<T> {
            type Value = Collection<T>;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an array of objects with unique ids")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let mut map = BTreeMap::new();
                while let Some(item) = seq.next_element::<T>()? {
                    let id = item.id();
                    if map.insert(id, item).is_some() {
                        return Err(de::Error::custom(format!("duplicate id {id}")));
                    }
                }
                Ok(Collection(map))
            }
        }

        deserializer.deserialize_seq(SeqVisitor(PhantomData))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Item {
        id: Id,
        n: u32,
    }

    impl Keyed for Item {
        fn id(&self) -> Id {
            self.id
        }
    }

    #[test]
    fn serializes_sorted_by_id() {
        let a = Item {
            id: Id::from_u128(2),
            n: 1,
        };
        let b = Item {
            id: Id::from_u128(1),
            n: 2,
        };
        let collection: Collection<Item> = [a.clone(), b.clone()].into_iter().collect();
        let json = serde_json::to_string(&collection).unwrap();
        let reversed: Collection<Item> = [b, a].into_iter().collect();
        assert_eq!(json, serde_json::to_string(&reversed).unwrap());
        assert!(json.find("\"n\":2").unwrap() < json.find("\"n\":1").unwrap());
        let back: Collection<Item> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, collection);
    }

    #[test]
    fn rejects_duplicate_ids() {
        let id = Id::from_u128(5);
        let json = serde_json::to_string(&vec![Item { id, n: 1 }, Item { id, n: 2 }]).unwrap();
        let error = serde_json::from_str::<Collection<Item>>(&json).unwrap_err();
        assert!(error.to_string().contains("duplicate id"));
    }

    #[test]
    fn basic_operations() {
        let mut collection = Collection::new();
        let id = Id::random();
        assert!(collection.insert(Item { id, n: 1 }).is_none());
        assert_eq!(collection.insert(Item { id, n: 2 }).unwrap().n, 1);
        assert!(collection.contains(id));
        collection.get_mut(id).unwrap().n = 3;
        assert_eq!(collection.get(id).unwrap().n, 3);
        assert_eq!(collection.len(), 1);
        assert_eq!(collection.ids().collect::<Vec<_>>(), vec![id]);
        assert_eq!(collection.remove(id).unwrap().n, 3);
        assert!(collection.is_empty());
    }
}
