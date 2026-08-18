/// The minimal set of metadata needed from routee-powertrain v2 to
/// integrate with Compass's prediction models.
pub mod routee_powertrain_v2_metadata {
    use serde::{Deserialize, Serialize};
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct Vehicle {
        pub mass_lbs: f64,
    }
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct Contract {
        pub feature_set: Vec<Feature>,
        pub real_world_adjustment_factor: f64,
    }
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct Estimator {
        pub model_file: String,
    }
    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct Feature {
        pub name: String,
        pub units: String,
        pub dtype: String,
        pub constraints: Constraints, // { lower: Option<f64>, upper: Option<f64> }
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct Constraints {
        pub lower: Option<f64>,
        pub upper: Option<f64>,
    }
}
