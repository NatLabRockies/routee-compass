use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Helper type that can deserialize either a single item or a vector of items
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum OneOrMany<T: Clone> {
    /// first attempt: this is a Vector of T
    Many(Vec<T>),
    /// second attempt: this is a single instance of T
    One(T),
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

impl<T> JsonSchema for OneOrMany<T> where T: JsonSchema + Clone {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        format!("OneOrMany_{}", T::schema_name()).into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "anyOf": [
                generator.subschema_for::<T>(), 
                generator.subschema_for::<Vec<T>>()
            ],
            "description": "Either a single item or an array of items"
        })
    }
}