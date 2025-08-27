use geo::Geometry;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};

use super::deserialize_geometry;
use super::{OvertureMapsBbox, OvertureMapsNames, OvertureMapsSource, OvertureRecord};
use crate::collection::OvertureMapsCollectionError;

/// OvertureMaps Places record schema according to schema of example parquet
/// The schema online does not mention some of the fields are nullable
#[allow(dead_code)]
#[derive(Debug, Serialize, Deserialize)]
pub struct PlacesRecord {
    #[serde(deserialize_with = "deserialize_geometry")]
    geometry: Option<Geometry>,
    categories: Option<OvertureMapsPlacesCategories>,
    id: Option<String>,
    bbox: OvertureMapsBbox,
    version: i32,
    sources: Option<Vec<Option<OvertureMapsSource>>>,
    names: OvertureMapsNames,
    confidence: Option<f32>,
    websites: Option<Vec<String>>,
    socials: Option<Vec<String>>,
    emails: Option<Vec<String>>,
    phones: Option<Vec<String>>,
    brand: Option<OvertureMapsPlacesBrand>,
    addresses: Option<Vec<OvertureMapsPlacesAddresses>>,
}

impl TryFrom<OvertureRecord> for PlacesRecord {
    type Error = OvertureMapsCollectionError;

    fn try_from(value: OvertureRecord) -> Result<Self, Self::Error> {
        match value {
            OvertureRecord::Places(record) => Ok(record),
            _ => Err(OvertureMapsCollectionError::DeserializeTypeError(format!(
                "Cannot transform record {value:#?} into PlacesRecord"
            ))),
        }
    }
}

impl fmt::Display for PlacesRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let categories = self
            .categories
            .as_ref()
            .map(|vec| format!("{vec:?}"))
            .unwrap_or("None".to_string());
        write!(
            f,
            "Geometry: {:#?}, Categories: {}",
            self.geometry, categories
        )
    }
}

impl PlacesRecord {
    pub fn get_categories(&self) -> Vec<String> {
        match &self.categories {
            Some(categories) => categories.linearize(),
            None => vec![],
        }
    }

    pub fn get_geometry(&self) -> Option<Geometry> {
        self.geometry.clone()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct OvertureMapsPlacesCategories {
    primary: Option<String>,
    alternate: Option<Vec<Option<String>>>,
}

impl OvertureMapsPlacesCategories {
    fn linearize(&self) -> Vec<String> {
        // TODO This could be more idiomatic
        let mut result = vec![];
        if let Some(primary) = &self.primary {
            result.push(primary.clone());
        }

        if let Some(alternate) = &self.alternate {
            alternate.iter().for_each(|maybe_str| {
                if let Some(str) = maybe_str {
                    result.push(str.clone());
                }
            });
        }

        result
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct OvertureMapsPlacesBrand {
    wikidata: Option<String>,
    names: Option<OvertureMapsPlacesBrandName>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OvertureMapsPlacesBrandName {
    primary: Option<String>,
    common: Option<HashMap<String, Option<String>>>,
    rules: Option<Vec<OvertureMapsPlacesBrandNameRule>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OvertureMapsPlacesBrandNameRule {
    variant: Option<String>,
    language: Option<String>,
    value: Option<String>,
    between: Option<Vec<String>>,
    side: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OvertureMapsPlacesAddresses {
    freeform: Option<String>,
    locality: Option<String>,
    postcode: Option<String>,
    region: Option<String>,
    country: Option<String>,
}
