use super::Bbox;
use crate::{
    app::CliBoundingBox,
    collection::{OvertureMapsCollectionError, TaxonomyModelBuilder},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    mem::discriminant,
    ops::Deref,
};

#[allow(unused)]
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum RowFilterConfig {
    HasClass,
    HasClassIn {
        classes: HashSet<String>,
    },
    Bbox {
        xmin: f32,
        xmax: f32,
        ymin: f32,
        ymax: f32,
    },
    TaxonomyModel {
        taxonomy_builder: TaxonomyModelBuilder,
    },
    Combined {
        filters: Vec<Box<RowFilterConfig>>,
    },
}

impl RowFilterConfig {
    /// If a combined filter is defined, we want to enforce the constraint
    /// that at most one of the filters types is used
    pub fn validate_unique_variant(&self) -> Result<(), OvertureMapsCollectionError> {
        match self {
            Self::Combined { filters } => {
                let mut seen_variants: HashSet<std::mem::Discriminant<RowFilterConfig>> =
                    HashSet::new();
                filters
                    .iter()
                    .all(|e| seen_variants.insert(discriminant(e.deref())));

                if seen_variants.len() != filters.len() {
                    return Err(OvertureMapsCollectionError::InvalidUserInput(format!("each row filter can be implemented at most once in a Combined row filter config: {self:?}")));
                }

                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// returns the bounding box associated with any bbox filter
    /// if available
    pub fn get_bbox_filter_if_exists(&self) -> Option<Bbox> {
        match self {
            Self::Bbox {
                xmin,
                xmax,
                ymin,
                ymax,
            } => Some(Bbox {
                xmin: *xmin,
                xmax: *xmax,
                ymin: *ymin,
                ymax: *ymax,
            }),
            Self::Combined { filters } => {
                for box_filter in filters {
                    if let Self::Bbox {
                        xmin,
                        xmax,
                        ymin,
                        ymax,
                    } = box_filter.deref()
                    {
                        return Some(Bbox {
                            xmin: *xmin,
                            xmax: *xmax,
                            ymin: *ymin,
                            ymax: *ymax,
                        });
                    };
                }
                None
            }
            _ => None,
        }
    }
}

impl From<HashMap<String, Vec<String>>> for RowFilterConfig {
    fn from(value: HashMap<String, Vec<String>>) -> Self {
        RowFilterConfig::TaxonomyModel {
            taxonomy_builder: TaxonomyModelBuilder::from(value),
        }
    }
}

impl From<Bbox> for RowFilterConfig {
    fn from(value: Bbox) -> Self {
        RowFilterConfig::Bbox {
            xmin: value.xmin,
            xmax: value.xmax,
            ymin: value.ymin,
            ymax: value.ymax,
        }
    }
}

impl From<&CliBoundingBox> for RowFilterConfig {
    fn from(value: &CliBoundingBox) -> Self {
        Self::Bbox {
            xmin: value.xmin,
            xmax: value.xmax,
            ymin: value.ymin,
            ymax: value.ymax,
        }
    }
}
