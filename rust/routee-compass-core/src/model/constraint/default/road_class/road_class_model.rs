use super::road_class_service::RoadClassFrontierService;
use crate::model::{
    constraint::{ConstraintModel, ConstraintModelError},
    network::Edge,
    state::{StateModel, StateVariable},
};
use std::{collections::HashSet, sync::Arc};

pub struct RoadClassConstraintModel {
    pub service: Arc<RoadClassFrontierService>,
    pub query_road_classes: Option<HashSet<u8>>,
}

impl ConstraintModel for RoadClassConstraintModel {
    fn valid_frontier(
        &self,
        edge: &Edge,
        _previous_edge: Option<&Edge>,
        _state: &[StateVariable],
        _state_model: &StateModel,
    ) -> Result<bool, ConstraintModelError> {
        self.valid_edge(edge)
    }

    fn valid_edge(&self, edge: &Edge) -> Result<bool, ConstraintModelError> {
        match &self.query_road_classes {
            None => Ok(true),
            Some(road_classes) => self
                .service
                .road_class_by_edge
                .get(edge.edge_id.0)
                .ok_or_else(|| {
                    ConstraintModelError::ConstraintModelError(format!(
                        "edge id {} missing from constraint model file",
                        edge.edge_id
                    ))
                })
                .map(|road_class| road_classes.contains(road_class)),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::model::{
        constraint::{
            default::road_class::road_class_service::RoadClassFrontierService, ConstraintModel,
            ConstraintModelService,
        },
        network::Edge,
        state::StateModel,
    };
    use serde_json::{json, Value};
    use std::sync::Arc;
    use uom::si::f64::Length;

    /// builds the test model for a given RoadClassModel test
    /// # Arguments
    /// * `road_class_vector` - the value assumed to be read from a file, with road classes by EdgeId index value
    /// * `query` - the user query which should provide the set of valid road classes for this search
    fn mock(road_class_vector: &[String], query: Value) -> Arc<dyn ConstraintModel> {
        let mut mapping = std::collections::HashMap::new();
        let mut encoded = Vec::with_capacity(road_class_vector.len());
        let mut next_id = 0u8;

        for class in road_class_vector.iter() {
            let id = *mapping.entry(class.clone()).or_insert_with(|| {
                let id = next_id;
                next_id += 1;
                id
            });
            encoded.push(id);
        }

        let service = Arc::new(RoadClassFrontierService {
            road_class_by_edge: Arc::new(encoded.into_boxed_slice()),
            road_class_mapping: Arc::new(mapping),
        });
        let state_model = Arc::new(StateModel::empty());
        service.build(&query, state_model.clone()).unwrap()
    }

    fn mock_edge() -> Edge {
        Edge::new(0, 0, 0, 1, Length::new::<uom::si::length::meter>(1.0))
    }

    #[test]
    fn test_no_road_classes() {
        let model = mock(&[String::from("a")], json!({}));
        let edge = mock_edge();
        let result = model.valid_edge(&edge).unwrap();
        assert!(result)
    }

    #[test]
    fn test_valid_class() {
        let model = mock(&[String::from("a")], json!({"road_classes": ["a"]}));
        let edge = mock_edge();
        let result = model.valid_edge(&edge).unwrap();
        assert!(result)
    }

    #[test]
    fn test_invalid_class() {
        let road_class_vector = Box::new([String::from("oh no!")]);
        let mut mapping = std::collections::HashMap::new();
        let mut encoded = Vec::with_capacity(road_class_vector.len());
        let mut next_id = 0u8;

        for class in road_class_vector.iter() {
            let id = *mapping.entry(class.clone()).or_insert_with(|| {
                let id = next_id;
                next_id += 1;
                id
            });
            encoded.push(id);
        }

        let service = Arc::new(RoadClassFrontierService {
            road_class_by_edge: Arc::new(encoded.into_boxed_slice()),
            road_class_mapping: Arc::new(mapping),
        });
        let state_model = Arc::new(StateModel::empty());
        let result = service.build(&json!({"road_classes": ["a"]}), state_model.clone());
        assert!(result.is_err());
        if let Err(e) = result {
            let err_msg = e.to_string();
            assert!(err_msg.contains("road class 'a' not found in road class mapping"));
        }
    }

    #[test]
    fn test_one_of_valid_classes() {
        let road_class_vector = Box::new([String::from("a")]);
        let mut mapping = std::collections::HashMap::new();
        let mut encoded = Vec::with_capacity(road_class_vector.len());
        let mut next_id = 0u8;

        for class in road_class_vector.iter() {
            let id = *mapping.entry(class.clone()).or_insert_with(|| {
                let id = next_id;
                next_id += 1;
                id
            });
            encoded.push(id);
        }

        let service = Arc::new(RoadClassFrontierService {
            road_class_by_edge: Arc::new(encoded.into_boxed_slice()),
            road_class_mapping: Arc::new(mapping),
        });
        let state_model = Arc::new(StateModel::empty());
        let result = service.build(
            &json!({"road_classes": ["a", "b", "c"]}),
            state_model.clone(),
        );
        assert!(result.is_err());
        if let Err(e) = result {
            let err_msg = e.to_string();
            assert!(err_msg.contains("not found in road class mapping"));
        }
    }

    #[test]
    fn test_none_of_valid_classes() {
        let road_class_vector = Box::new([String::from("oh no!")]);
        let mut mapping = std::collections::HashMap::new();
        let mut encoded = Vec::with_capacity(road_class_vector.len());
        let mut next_id = 0u8;

        for class in road_class_vector.iter() {
            let id = *mapping.entry(class.clone()).or_insert_with(|| {
                let id = next_id;
                next_id += 1;
                id
            });
            encoded.push(id);
        }

        let service = Arc::new(RoadClassFrontierService {
            road_class_by_edge: Arc::new(encoded.into_boxed_slice()),
            road_class_mapping: Arc::new(mapping),
        });
        let state_model = Arc::new(StateModel::empty());
        let result = service.build(
            &json!({"road_classes": ["a", "b", "c"]}),
            state_model.clone(),
        );
        assert!(result.is_err());
        if let Err(e) = result {
            let err_msg = e.to_string();
            assert!(err_msg.contains("not found in road class mapping"));
        }
    }

    #[test]
    fn test_valid_numeric_class() {
        let model = mock(&[String::from("1")], json!({"road_classes": [1]}));
        let edge = mock_edge();
        let result = model.valid_edge(&edge).unwrap();
        assert!(result)
    }

    #[test]
    fn test_invalid_numeric_class() {
        let road_class_vector = Box::new([String::from("OH NO!")]);
        let mut mapping = std::collections::HashMap::new();
        let mut encoded = Vec::with_capacity(road_class_vector.len());
        let mut next_id = 0u8;

        for class in road_class_vector.iter() {
            let id = *mapping.entry(class.clone()).or_insert_with(|| {
                let id = next_id;
                next_id += 1;
                id
            });
            encoded.push(id);
        }

        let service = Arc::new(RoadClassFrontierService {
            road_class_by_edge: Arc::new(encoded.into_boxed_slice()),
            road_class_mapping: Arc::new(mapping),
        });
        let state_model = Arc::new(StateModel::empty());
        let result = service.build(&json!({"road_classes": [1]}), state_model.clone());
        assert!(result.is_err());
        if let Err(e) = result {
            let err_msg = e.to_string();
            assert!(err_msg.contains("road class '1' not found in road class mapping"));
        }
    }

    #[test]
    fn test_valid_boolean_class() {
        let model = mock(&[String::from("true")], json!({"road_classes": [true]}));
        let edge = mock_edge();
        let result = model.valid_edge(&edge).unwrap();
        assert!(result)
    }

    #[test]
    fn test_invalid_boolean_class() {
        let road_class_vector = Box::new([String::from("OH NO!")]);
        let mut mapping = std::collections::HashMap::new();
        let mut encoded = Vec::with_capacity(road_class_vector.len());
        let mut next_id = 0u8;

        for class in road_class_vector.iter() {
            let id = *mapping.entry(class.clone()).or_insert_with(|| {
                let id = next_id;
                next_id += 1;
                id
            });
            encoded.push(id);
        }

        let service = Arc::new(RoadClassFrontierService {
            road_class_by_edge: Arc::new(encoded.into_boxed_slice()),
            road_class_mapping: Arc::new(mapping),
        });
        let state_model = Arc::new(StateModel::empty());
        let result = service.build(&json!({"road_classes": [true]}), state_model.clone());
        assert!(result.is_err());
        if let Err(e) = result {
            let err_msg = e.to_string();
            assert!(err_msg.contains("road class 'true' not found in road class mapping"));
        }
    }

    #[test]
    fn test_edge_with_different_valid_class_is_rejected() {
        // Both "a" and "b" are in the mapping, but edge has "b" and query wants "a"
        let model = mock(
            &[String::from("b"), String::from("a")],
            json!({"road_classes": ["a"]}),
        );
        let edge = mock_edge(); // edge 0 has road class "b"
        let result = model.valid_edge(&edge).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_edge_matches_one_of_multiple_valid_classes() {
        // Edge has "a", query allows "a", "b", "c" - all are in mapping
        let model = mock(
            &[String::from("a"), String::from("b"), String::from("c")],
            json!({"road_classes": ["a", "b", "c"]}),
        );
        let edge = mock_edge(); // edge 0 has road class "a"
        let result = model.valid_edge(&edge).unwrap();
        assert!(result);
    }

    #[test]
    fn test_edge_rejected_when_not_in_multiple_valid_classes() {
        // Edge has "d", query allows "a", "b", "c" - all are in mapping
        let model = mock(
            &[
                String::from("d"),
                String::from("a"),
                String::from("b"),
                String::from("c"),
            ],
            json!({"road_classes": ["a", "b", "c"]}),
        );
        let edge = mock_edge(); // edge 0 has road class "d"
        let result = model.valid_edge(&edge).unwrap();
        assert!(!result);
    }
}
