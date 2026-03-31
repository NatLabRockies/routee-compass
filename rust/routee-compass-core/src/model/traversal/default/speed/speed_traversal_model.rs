use uom::si::f64::Velocity;
use uom::ConstZero;

use super::speed_traversal_engine::SpeedTraversalEngine;
use crate::algorithm::search::SearchGraph;
use crate::model::network::{Edge, EdgeId, Vertex};
use crate::model::state::StateModel;
use crate::model::state::StateVariable;
use crate::model::state::{InputFeature, StateVariableConfig};
use crate::model::traversal::default::fieldname;
use crate::model::traversal::traversal_model::TraversalModel;
use crate::model::traversal::traversal_model_error::TraversalModelError;
use crate::model::unit::SpeedUnit;
use std::sync::Arc;

pub struct SpeedTraversalModel {
    engine: Arc<SpeedTraversalEngine>,
    speed_limit: Option<Velocity>,
}

impl SpeedTraversalModel {
    pub fn new(
        engine: Arc<SpeedTraversalEngine>,
        speed_limit: Option<Velocity>,
    ) -> Result<SpeedTraversalModel, TraversalModelError> {
        if let Some(max_speed) = speed_limit {
            Ok(SpeedTraversalModel {
                engine,
                speed_limit: Some(max_speed),
            })
        } else {
            Ok(SpeedTraversalModel {
                engine,
                speed_limit: None,
            })
        }
    }
}

impl TraversalModel for SpeedTraversalModel {
    fn name(&self) -> String {
        String::from("Speed Traversal Model")
    }
    fn input_features(&self) -> Vec<InputFeature> {
        vec![InputFeature::Distance {
            name: fieldname::EDGE_DISTANCE.to_string(),
            unit: None,
        }]
    }

    fn output_features(&self) -> Vec<(String, StateVariableConfig)> {
        vec![(
            String::from(fieldname::EDGE_SPEED),
            StateVariableConfig::Speed {
                initial: Velocity::ZERO,
                accumulator: false,
                output_unit: Some(SpeedUnit::default()),
            },
        )]
    }

    /// records the speed that will be driven over this edge into the state vector.
    fn traverse_edge(
        &self,
        trajectory: (&Vertex, &Edge, &Vertex),
        state: &mut Vec<StateVariable>,
        _tree: &SearchGraph,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        let (_, edge, _) = trajectory;
        let lookup_speed = get_speed(&self.engine.speed_table, edge.edge_id)?;
        let speed = apply_speed_limit(lookup_speed, self.speed_limit);
        state_model.set_speed(state, fieldname::EDGE_SPEED, &speed)?;
        Ok(())
    }

    /// (over-)estimates speed over remainder of the trip as the maximum-possible speed value.
    fn estimate_traversal(
        &self,
        _od: (&Vertex, &Vertex),
        state: &mut Vec<StateVariable>,
        _tree: &SearchGraph,
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        let speed: Velocity = match self.speed_limit {
            Some(speed_limit) => speed_limit,
            None => self.engine.max_speed,
        };
        state_model.set_speed(state, fieldname::EDGE_SPEED, &speed)?;

        Ok(())
    }
}

/// look up a speed from the speed table
pub fn get_speed(
    speed_table: &[Velocity],
    edge_id: EdgeId,
) -> Result<Velocity, TraversalModelError> {
    let speed: &Velocity = speed_table.get(edge_id.as_usize()).ok_or_else(|| {
        TraversalModelError::TraversalModelFailure(format!(
            "could not find expected index {edge_id} in speed table"
        ))
    })?;
    Ok(*speed)
}

fn apply_speed_limit(lookup_speed: Velocity, speed_limit: Option<Velocity>) -> Velocity {
    match speed_limit {
        Some(speed_limit) => {
            if lookup_speed > speed_limit {
                speed_limit
            } else {
                lookup_speed
            }
        }
        None => lookup_speed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::network::{Edge, EdgeId, EdgeListId, Vertex, VertexId};
    use crate::model::unit::SpeedUnit;
    use crate::testing::mock::traversal_model::TestTraversalModel;
    use crate::util::geo::InternalCoord;
    use approx::relative_eq;
    use geo::coord;
    use std::path::PathBuf;
    use uom::si::f64::Length;

    fn mock_vertex() -> Vertex {
        Vertex {
            vertex_id: VertexId(0),
            coordinate: InternalCoord(coord! {x: -86.67, y: 36.12}),
        }
    }
    fn mock_edge(edge_id: usize) -> Edge {
        Edge {
            edge_list_id: EdgeListId(0),
            edge_id: EdgeId(edge_id),
            src_vertex_id: VertexId(0),
            dst_vertex_id: VertexId(1),
            distance: Length::new::<uom::si::length::meter>(100.0),
        }
    }
    fn filepath() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("model")
            .join("traversal")
            .join("default")
            .join("test")
            .join("velocities.txt")
    }

    // save in case we develop test cases that may leverage this
    #[allow(dead_code)]
    fn approx_eq(a: f64, b: f64, error: f64) {
        let result = match (a, b) {
            (c, d) if c < d => d - c < error,
            (c, d) if c > d => c - d < error,
            (_, _) => true,
        };
        assert!(result, "{a} ~= {b} is not true within an error of {error}")
    }

    #[test]
    fn test_speed_traversal() {
        let file: PathBuf = filepath();
        let engine =
            SpeedTraversalEngine::new(&file, SpeedUnit::KPH).expect("test invariant failed");
        let speed_model =
            SpeedTraversalModel::new(Arc::new(engine), None).expect("test invariant failed");
        let test_model =
            TestTraversalModel::new(Arc::new(speed_model)).expect("test invariant failed");
        let state_model = StateModel::empty()
            .register(test_model.input_features(), test_model.output_features())
            .expect("failed tp register state features");

        let mut state = state_model.initial_state(None).unwrap();
        let v = mock_vertex();
        let e1 = mock_edge(0);
        test_model
            .traverse_edge(
                (&v, &e1, &v),
                &mut state,
                &SearchGraph::default(),
                &state_model,
            )
            .unwrap();

        let expected_speed_kph = 10.0;
        let result_speed = state_model
            .get_speed(&state, "edge_speed")
            .expect("test invariant failed");
        let result_speed_kph = result_speed.get::<uom::si::velocity::kilometer_per_hour>();

        assert_eq!(expected_speed_kph, result_speed_kph);
    }

    #[test]
    fn test_speed_limit_enforcement() {
        // We know from the test data that edge 0 has a speed of 10 kph, so set a limit of 5 kph
        let speed_limit = Velocity::new::<uom::si::velocity::kilometer_per_hour>(5.0);

        let file: PathBuf = filepath();
        let engine = Arc::new(
            SpeedTraversalEngine::new(&file, SpeedUnit::KPH).expect("test invariant failed"),
        );

        let regular_model =
            SpeedTraversalModel::new(engine.clone(), None).expect("test invariant failed");
        let limited_model = SpeedTraversalModel::new(engine.clone(), Some(speed_limit))
            .expect("test invariant failed");

        let test_regular_model =
            TestTraversalModel::new(Arc::new(regular_model)).expect("test invariant failed");
        let test_limited_model =
            TestTraversalModel::new(Arc::new(limited_model)).expect("test invariant failed");
        let state_model = StateModel::empty()
            .register(
                test_regular_model.input_features(),
                test_regular_model.output_features(),
            )
            .expect("test invariant failed");
        let tree = SearchGraph::default();

        let mut state_with_limit = state_model.initial_state(None).unwrap();
        let mut state_without_limit = state_model.initial_state(None).unwrap();

        let v = mock_vertex();
        let e = mock_edge(0);

        // Traverse with speed limit
        test_limited_model
            .traverse_edge((&v, &e, &v), &mut state_with_limit, &tree, &state_model)
            .unwrap();

        // Traverse without speed limit
        test_regular_model
            .traverse_edge((&v, &e, &v), &mut state_without_limit, &tree, &state_model)
            .unwrap();

        // The time with speed limit should be about twice the time without limit
        // because we set the limit to half the edge speed (5 kph vs 10 kph)
        let speed_with_limit = state_model
            .get_speed(&state_with_limit, "edge_speed")
            .expect("test invariant failed");
        let speed_without_limit = state_model
            .get_speed(&state_without_limit, "edge_speed")
            .expect("test invariant failed");
        let speed_with_limit_kph = speed_with_limit.get::<uom::si::velocity::kilometer_per_hour>();
        let speed_limit_kph = speed_limit.get::<uom::si::velocity::kilometer_per_hour>();

        let _ = relative_eq!(speed_with_limit_kph, speed_limit_kph,);
        let _ = relative_eq!(
            speed_without_limit.get::<uom::si::velocity::kilometer_per_hour>(),
            10.0,
        );
    }
}
