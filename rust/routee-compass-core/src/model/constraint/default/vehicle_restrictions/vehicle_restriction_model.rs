use super::{VehicleParameter, VehicleRestrictionFrontierService};
use crate::model::{
    constraint::{ConstraintModel, ConstraintModelError},
    network::Edge,
    state::{StateModel, StateVariable},
};
use std::sync::Arc;

pub struct VehicleRestrictionConstraintModel {
    pub service: Arc<VehicleRestrictionFrontierService>,
    pub vehicle_parameters: Vec<VehicleParameter>,
}

impl ConstraintModel for VehicleRestrictionConstraintModel {
    fn valid_frontier(
        &self,
        edge: &Edge,
        _previos_edge: Option<&Edge>,
        _state: &[StateVariable],
        _state_model: &StateModel,
    ) -> Result<bool, ConstraintModelError> {
        validate_edge(self, edge)
    }

    fn valid_edge(&self, edge: &Edge) -> Result<bool, ConstraintModelError> {
        validate_edge(self, edge)
    }
}

fn validate_edge(
    model: &VehicleRestrictionConstraintModel,
    edge: &Edge,
) -> Result<bool, ConstraintModelError> {
    // if there are no parameters or restrictions, the edge is valid
    let restriction_map_option = model.service.vehicle_restriction_lookup.get(&edge.edge_id);
    let restrictions = match (restriction_map_option, model.vehicle_parameters.as_slice()) {
        (None, _) => return Ok(true), // no restrictions on edge
        (_, []) => return Ok(true),   // no vehicle parameters on query
        (Some(vehicle_restrictions), _) => vehicle_restrictions,
    };

    // for each parameter of this constraint model, test if the parameter is valid for any matching restriction
    for p in model.vehicle_parameters.iter() {
        let p_type = p.vehicle_parameter_type();
        match restrictions.get(p_type) {
            Some(r) if !r.within_restriction(p) => {
                return Ok(false);
            }
            _ => {}
        }
    }

    Ok(true)
}

#[cfg(test)]
mod test {

    use crate::model::{
        constraint::{
            default::vehicle_restrictions::VehicleRestrictionBuilder, ConstraintModel,
            ConstraintModelBuilder,
        },
        network::Edge,
        state::StateModel,
    };
    use serde_json::{json, Value};
    use std::{
        path::{Path, PathBuf},
        sync::Arc,
    };
    use uom::{si::f64::Length, ConstZero};

    fn mock_edge() -> Edge {
        Edge::new(0, 0, 0, 0, Length::ZERO)
    }

    #[test]
    fn test_e2e_valid_weight_and_height() {
        let model = build_model("test_restrictions.csv", "unrestricted.json");
        let edge = mock_edge();
        match model.valid_edge(&edge) {
            Ok(is_ok) => assert!(is_ok),
            Err(e) => panic!("{}", e),
        }
    }

    #[test]
    fn test_e2e_invalid_weight() {
        let model = build_model("test_restrictions.csv", "overweight.json");
        let edge = mock_edge();
        match model.valid_edge(&edge) {
            Ok(is_ok) => assert!(!is_ok),
            Err(e) => panic!("{}", e),
        }
    }

    #[test]
    fn test_e2e_invalid_height() {
        let model = build_model("test_restrictions.csv", "overheight.json");
        let edge = mock_edge();
        match model.valid_edge(&edge) {
            Ok(is_ok) => assert!(!is_ok),
            Err(e) => panic!("{}", e),
        }
    }

    #[test]
    fn test_e2e_unrestricted_parameter() {
        let model = build_model("test_restrictions.csv", "unknown_parameter.json");
        let edge = mock_edge();
        match model.valid_edge(&edge) {
            Ok(is_ok) => assert!(is_ok),
            Err(e) => panic!("{}", e),
        }
    }

    fn build_model(restriction_filename: &str, query_filename: &str) -> Arc<dyn ConstraintModel> {
        let restriction_file = test_filepath(restriction_filename);
        let conf = json!({
            "vehicle_restriction_input_file": restriction_file,
        });
        let query = read_json_file(query_filename);
        let service = VehicleRestrictionBuilder {}
            .build(&conf)
            .unwrap_or_else(|e| {
                panic!("failed to read test CSV {restriction_filename} due to: {e}")
            });
        let state_model = Arc::new(StateModel::new(vec![]));

        (service.build(&query, state_model).unwrap_or_else(|_| {
            panic!(
                "failed to build model from service with query: {}",
                &serde_json::to_string(&query).unwrap_or_default()
            )
        })) as _
    }

    fn read_json_file(filename: &str) -> Value {
        let filepath = test_filepath(filename);
        let file_contents = std::fs::read_to_string(&filepath)
            .unwrap_or_else(|_| panic!("test invariant failed, unable to load {}", &filepath));

        serde_json::from_str(&file_contents)
            .unwrap_or_else(|_| panic!("test invariant failed, unable to parse {}", &filepath))
    }

    fn test_filepath(filename: &str) -> String {
        let mut path = test_dir();
        path.push(filename);
        path.to_str()
            .unwrap_or_else(|| panic!("test invariant failed, unable to load {filename}"))
            .to_string()
    }

    fn test_dir() -> PathBuf {
        // rust/routee-compass/src/app/compass/model/constraint_model/vehicle_restrictions/test
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("model")
            .join("constraint")
            .join("default")
            .join("vehicle_restrictions")
            .join("test")
    }
}
