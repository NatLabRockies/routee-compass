use serde::{Deserialize, Serialize};

use crate::collection::OvertureMapsCollectionError;

#[derive(Clone, Debug, Serialize, Deserialize, Copy)]
pub struct Bbox {
    pub xmin: f32,
    pub xmax: f32,
    pub ymin: f32,
    pub ymax: f32,
}

impl Bbox {
    pub fn new(xmin: f32, xmax: f32, ymin: f32, ymax: f32) -> Self {
        Self {
            xmin,
            xmax,
            ymin,
            ymax,
        }
    }

    pub fn validate(&self) -> Result<(), OvertureMapsCollectionError> {
        if self.xmax < self.xmin || self.ymax < self.xmin {
            return Err(OvertureMapsCollectionError::InvalidUserInput(format!(
                "The provided Bbox is invalid: {self:?}"
            )));
        }

        Ok(())
    }
}
