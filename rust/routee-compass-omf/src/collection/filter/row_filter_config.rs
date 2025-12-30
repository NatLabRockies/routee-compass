use super::Bbox;
use crate::{app::CliBoundingBox, collection::TaxonomyModelBuilder};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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
