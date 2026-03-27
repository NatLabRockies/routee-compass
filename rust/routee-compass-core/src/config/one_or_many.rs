use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize, Serializer};
use std::fmt;
use std::marker::PhantomData;

/// Helper type that can deserialize either a single item or a vector of items
#[derive(Clone, Debug)]
pub enum OneOrMany<T: Clone> {
    /// first attempt: this is a Vector of T
    Many(Vec<T>),
    /// second attempt: this is a single instance of T
    One(T),
}

struct OneOrManyVisitor<T>(PhantomData<T>);

impl<'de, T: Clone + Deserialize<'de>> Visitor<'de> for OneOrManyVisitor<T> {
    type Value = OneOrMany<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a single item or a sequence of items")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element::<T>()? {
            items.push(item);
        }
        Ok(OneOrMany::Many(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        T::deserialize(de::value::MapAccessDeserializer::new(map)).map(OneOrMany::One)
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        T::deserialize(de::value::StrDeserializer::new(v)).map(OneOrMany::One)
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
        T::deserialize(de::value::StringDeserializer::new(v)).map(OneOrMany::One)
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
        T::deserialize(de::value::BoolDeserializer::new(v)).map(OneOrMany::One)
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
        T::deserialize(de::value::I64Deserializer::new(v)).map(OneOrMany::One)
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
        T::deserialize(de::value::U64Deserializer::new(v)).map(OneOrMany::One)
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
        T::deserialize(de::value::F64Deserializer::new(v)).map(OneOrMany::One)
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
        T::deserialize(de::value::UnitDeserializer::new()).map(OneOrMany::One)
    }
}

impl<'de, T: Clone + Deserialize<'de>> Deserialize<'de> for OneOrMany<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(OneOrManyVisitor(PhantomData))
    }
}

impl<T: Clone + Serialize> Serialize for OneOrMany<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            OneOrMany::One(item) => item.serialize(serializer),
            OneOrMany::Many(items) => items.serialize(serializer),
        }
    }
}

impl<T: Clone> OneOrMany<T> {
    /// Convert to a vector, regardless of whether it was originally one item or many
    pub fn into_vec(self) -> Vec<T> {
        match self {
            OneOrMany::One(item) => vec![item],
            OneOrMany::Many(items) => items,
        }
    }

    /// Get a reference to the items as a vector
    pub fn as_vec(&self) -> Vec<&T> {
        match self {
            OneOrMany::One(item) => vec![item],
            OneOrMany::Many(items) => items.iter().collect(),
        }
    }

    pub fn to_vec(&self) -> Vec<T> {
        match self {
            OneOrMany::One(item) => vec![item.clone()],
            OneOrMany::Many(items) => items.to_vec(),
        }
    }

    /// Get the number of items
    pub fn len(&self) -> usize {
        match self {
            OneOrMany::One(_) => 1,
            OneOrMany::Many(items) => items.len(),
        }
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        match self {
            OneOrMany::One(_) => false,
            OneOrMany::Many(items) => items.is_empty(),
        }
    }

    /// Iterate over the items
    pub fn iter(&self) -> Box<dyn Iterator<Item = &T> + '_> {
        match self {
            OneOrMany::One(item) => Box::new(std::iter::once(item)),
            OneOrMany::Many(items) => Box::new(items.iter()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// this type is used in OneOrMany<T> tests as the provided T type.
    #[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
    struct Item {
        name: String,
        value: u32,
    }

    impl Item {
        /// create an item.
        fn new(name: &str, value: u32) -> Item {
            Item {
                name: name.to_string(),
                value,
            }
        }
    }

    #[test]
    fn test_deserialize_one() {
        let json = r#"{"name": "foo", "value": 42}"#;
        let result: OneOrMany<Item> = serde_json::from_str(json).unwrap();
        match result {
            OneOrMany::One(i) => assert_eq!(i, Item::new("foo", 42)),
            OneOrMany::Many(_) => panic!("expected One"),
        }
    }

    #[test]
    fn test_deserialize_many() {
        let json = r#"[{"name": "foo", "value": 1}, {"name": "bar", "value": 2}]"#;
        let result: OneOrMany<Item> = serde_json::from_str(json).unwrap();
        match result {
            OneOrMany::Many(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], Item::new("foo", 1));
                assert_eq!(items[1], Item::new("bar", 2));
            }
            OneOrMany::One(_) => panic!("expected Many"),
        }
    }

    #[test]
    fn test_deserialize_many_single_element() {
        // A JSON array with one element should still produce Many
        let json = r#"[{"name": "foo", "value": 1}]"#;
        let result: OneOrMany<Item> = serde_json::from_str(json).unwrap();
        match result {
            OneOrMany::Many(items) => assert_eq!(items.len(), 1),
            OneOrMany::One(_) => panic!("expected Many"),
        }
    }

    #[test]
    fn test_deserialize_empty_array() {
        let json = r#"[]"#;
        let result: OneOrMany<Item> = serde_json::from_str(json).unwrap();
        match result {
            OneOrMany::Many(items) => assert!(items.is_empty()),
            OneOrMany::One(_) => panic!("expected Many"),
        }
    }

    #[test]
    fn test_serialize_one_roundtrip() {
        let item = Item::new("foo", 42);
        let original = OneOrMany::One(item.clone());
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(json, r#"{"name":"foo","value":42}"#);
        let roundtripped: OneOrMany<Item> = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtripped.into_vec(), vec![item]);
    }

    #[test]
    fn test_serialize_many_roundtrip() {
        let items = vec![Item::new("foo", 1), Item::new("bar", 2)];
        let original = OneOrMany::Many(items.clone());
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(
            json,
            r#"[{"name":"foo","value":1},{"name":"bar","value":2}]"#
        );
        let roundtripped: OneOrMany<Item> = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtripped.into_vec(), items);
    }

    #[test]
    fn test_error_missing_field_in_one() {
        // "value" field is missing — error should name the missing field, not say "untagged enum"
        let json = r#"{"name": "foo"}"#;
        let err = serde_json::from_str::<OneOrMany<Item>>(json)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("value"),
            "expected error to mention the missing field 'value', got: {err}"
        );
    }

    #[test]
    fn test_error_wrong_type_in_one() {
        // "value" is a string instead of u32
        let json = r#"{"name": "foo", "value": "not-a-number"}"#;
        let err = serde_json::from_str::<OneOrMany<Item>>(json)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("expected u32"),
            "inner error message not captured"
        );
    }

    #[test]
    fn test_error_missing_field_in_many_element() {
        // Second element in array is missing "value" — error should propagate from element deserialization
        let json = r#"[{"name": "foo", "value": 1}, {"name": "bar"}]"#;
        let err = serde_json::from_str::<OneOrMany<Item>>(json)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("value"),
            "expected error to mention the missing field 'value', got: {err}"
        );
    }

    #[test]
    fn test_into_vec_one() {
        let one: OneOrMany<Item> = OneOrMany::One(Item::new("a", 1));
        assert_eq!(one.into_vec(), vec![Item::new("a", 1)]);
    }

    #[test]
    fn test_into_vec_many() {
        let many: OneOrMany<Item> = OneOrMany::Many(vec![Item::new("a", 1), Item::new("b", 2)]);
        assert_eq!(many.into_vec(), vec![Item::new("a", 1), Item::new("b", 2)]);
    }
}
