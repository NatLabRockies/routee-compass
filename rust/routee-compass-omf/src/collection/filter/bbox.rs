use serde::{Deserialize, Serialize};

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
}
