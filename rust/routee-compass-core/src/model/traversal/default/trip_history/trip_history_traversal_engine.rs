use super::TripHistoryTraversalConfig;
use crate::model::state::CustomVariableConfig;
use crate::model::state::{StateModel, StateModelError, StateVariable, StateVariableConfig};
use crate::model::traversal::default::trip_history::trip_history_traversal_config::HistoryFeature;
use crate::model::traversal::{EdgeFrontierContext, TraversalModelError};

/// Alias for feature name strings inside of `ShiftFeatureNameMappings`.
///
/// A feature name derived from the `TripHistoryTraversalModel` is `<feature_i>_<depth_j>`
/// Where `<feature_i>` is an alphabetic string representing the physcial feature name (e.g. "distance" or "time")
/// and `<depth_j>` is a numeric string that represents the depth backwards that the historical feature was pulled from in the tree.
type FeatureName = String;

/// The alias `ShiftFeatureNameMappings` is a vector map that maps all output feature names `<feature_i>_<depth_j>` to `<feature_i>_<depth_(j+1)>` for
/// use in the `shift()` algorithm in the `TripHistoryTraversalEngine`.
///
/// The first column of the vector is the original feature name, the next column is the target feature name for the shift algorithm, the third column
/// is the StateVariable type's StateVariableConfig
type ShiftFeatureNameMappings = Vec<(FeatureName, FeatureName, StateVariableConfig)>;

/// Given a set of `StateVariableConfig`s and a desired backtracking `depth`,
/// builds the vector mapping from feature name `"<variable_i>_<depth_j>"` to `"<variable_i>_<depth_(j+1)"`
/// and includes the feature's `StateVariableConfig`.
fn build_shift_feature_name_mappings(
    history_features: Vec<HistoryFeature>,
    depth: usize,
) -> Result<ShiftFeatureNameMappings, String> {
    let depths = (1..=depth - 1).rev();
    let mapping = depths
        .flat_map(|d| {
            history_features.iter().map(move |feature| {
                (
                    format!("{}_{}", feature.name, d),
                    format!("{}_{}", feature.name, d + 1),
                    feature.state_variable_config.clone(),
                )
            })
        })
        .collect::<Vec<(FeatureName, FeatureName, StateVariableConfig)>>();
    Ok(mapping)
}
/// The `TripHistoryTraversalEngine` is responsible for managing and updating historical trip features
/// within the state, using the set of input history features (e.g., "edge_distance", "edge_time", etc.) and the desired depth backwards in the history of the tree.
pub struct TripHistoryTraversalEngine {
    pub history_features: Vec<HistoryFeature>,
    pub depth: std::num::NonZeroUsize,
    pub shift_feature_name_mappings: ShiftFeatureNameMappings,
}

impl TryFrom<TripHistoryTraversalConfig> for TripHistoryTraversalEngine {
    type Error = TraversalModelError;

    fn try_from(config: TripHistoryTraversalConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            history_features: config.history_features.clone(),
            depth: config.depth,
            shift_feature_name_mappings: build_shift_feature_name_mappings(
                config.history_features,
                config.depth.get(),
            )
            .map_err(|e| {
                TraversalModelError::BuildError(format!(
                    "Failed to build ShiftFeatureNameMappings because of {e}"
                ))
            })?,
        })
    }
}

impl TripHistoryTraversalEngine {
    /// Updates the history features in the given state by shifting existing values and inserting the latest value from the context.
    pub fn update_history(
        &self,
        ctx: &EdgeFrontierContext,
        state: &mut [StateVariable],
        state_model: &StateModel,
    ) -> Result<(), TraversalModelError> {
        for (src, dst, conf) in self.shift_feature_name_mappings.iter() {
            self.shift(state, state_model, src, dst, conf)?;
        }
        for feature in self.history_features.iter() {
            self.insert_first(
                ctx,
                state,
                state_model,
                &feature.name,
                &feature.state_variable_config,
            )?;
        }
        Ok(())
    }
    /// Takes feature with name `{feature_i}_{depth_j}` and shifts its value to the value of feature with name `{feature_i}_{depth_(j+1)}`
    /// shifting values one link into "the past".
    pub fn shift(
        &self,
        state: &mut [StateVariable],
        state_model: &StateModel,
        src: &FeatureName,
        dst: &FeatureName,
        conf: &StateVariableConfig,
    ) -> Result<(), StateModelError> {
        // <feature_i>_<depth_j> = <feature_i>_<depth_j+1>;
        copy_state_variable(conf, state_model, state, None, dst, src)?;
        Ok(())
    }

    /// Traverse one step into the history via `ctx.tree.backtrack_with_depth(state_variable, depth)`
    /// and record the value at `format!({feature_name}_1")`. This must be run after `shift` to avoid
    /// overwriting the first history value before shifting. if the backtrack result is empty,
    /// do nothing.
    fn insert_first(
        &self,
        ctx: &EdgeFrontierContext,
        state: &mut [StateVariable],
        state_model: &StateModel,
        feature_name: &str,
        conf: &StateVariableConfig,
    ) -> Result<(), TraversalModelError> {
        let previous_edge = ctx.tree.backtrack_with_depth(ctx.src.vertex_id, 1)?;
        let previous_state: &[StateVariable] = &previous_edge[0].result_state;
        copy_state_variable(
            conf,
            state_model,
            state,
            Some(previous_state),
            &format!("{feature_name}_1"),
            feature_name,
        )?;
        Ok(())
    }
}

/// Copies a state variable from state `state` with feature name `read_name` into a state variable from state `state` with feature name `write_name`.
///
/// If `read_state` is specified, we can choose a different `&[StateVariable]` to read from. In this case, variables are still
/// written to `state`.
fn copy_state_variable(
    conf: &StateVariableConfig,
    state_model: &StateModel,
    state: &mut [StateVariable], // The state vector write to. Also the state vector to read from if `read_state` not specified.
    read_state: Option<&[StateVariable]>, // Optionally, the state vector to read from
    write_name: &str,
    read_name: &str,
) -> Result<(), StateModelError> {
    match conf {
        StateVariableConfig::Distance { .. } => {
            let v = state_model.get_distance(read_state.unwrap_or(state), read_name)?;
            state_model.set_distance(state, write_name, &v)
        }
        StateVariableConfig::Time { .. } => {
            let v = state_model.get_time(read_state.unwrap_or(state), read_name)?;
            state_model.set_time(state, write_name, &v)
        }
        StateVariableConfig::Speed { .. } => {
            let v = state_model.get_speed(read_state.unwrap_or(state), read_name)?;
            state_model.set_speed(state, write_name, &v)
        }
        StateVariableConfig::Energy { .. } => {
            let v = state_model.get_energy(read_state.unwrap_or(state), read_name)?;
            state_model.set_energy(state, write_name, &v)
        }
        StateVariableConfig::Ratio { .. } => {
            let v = state_model.get_ratio(read_state.unwrap_or(state), read_name)?;
            state_model.set_ratio(state, write_name, &v)
        }
        StateVariableConfig::Temperature { .. } => {
            let v = state_model.get_temperature(read_state.unwrap_or(state), read_name)?;
            state_model.set_temperature(state, write_name, &v)
        }
        StateVariableConfig::Custom { value, .. } => match value {
            CustomVariableConfig::FloatingPoint { .. } => {
                let v = state_model.get_custom_f64(read_state.unwrap_or(state), read_name)?;
                state_model.set_custom_f64(state, write_name, &v)
            }
            CustomVariableConfig::SignedInteger { .. } => {
                let v = state_model.get_custom_i64(read_state.unwrap_or(state), read_name)?;
                state_model.set_custom_i64(state, write_name, &v)
            }
            CustomVariableConfig::UnsignedInteger { .. } => {
                let v = state_model.get_custom_u64(read_state.unwrap_or(state), read_name)?;
                state_model.set_custom_u64(state, write_name, &v)
            }
            CustomVariableConfig::Boolean { .. } => {
                let v = state_model.get_custom_bool(read_state.unwrap_or(state), read_name)?;
                state_model.set_custom_bool(state, write_name, &v)
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use geo::Coord;
    use uom::si::f64::Length;

    use crate::{
        algorithm::search::{Direction, EdgeTraversal, SearchTree},
        model::{
            cost::TraversalCost,
            label::Label,
            network::{Edge, EdgeId, EdgeListId, Vertex, VertexId},
            state::{StateModel, StateVariable, StateVariableConfig},
            traversal::{
                default::trip_history::{
                    trip_history_traversal_config::HistoryFeature, TripHistoryTraversalConfig,
                    TripHistoryTraversalEngine,
                },
                EdgeFrontierContext,
            },
            unit::DistanceUnit,
        },
        util::geo::InternalCoord,
    };
    use std::{num::NonZeroUsize, sync::Arc};

    // Mock the trip history traversal engine
    fn mock_engine() -> Arc<TripHistoryTraversalEngine> {
        Arc::new(
            TripHistoryTraversalEngine::try_from(TripHistoryTraversalConfig {
                history_features: vec![HistoryFeature {
                    name: "edge_distance".to_string(),
                    state_variable_config: StateVariableConfig::Distance {
                        output_unit: Some(DistanceUnit::Miles),
                        initial: Length::default(),
                        accumulator: bool::default(),
                    },
                }],
                depth: NonZeroUsize::new(3).unwrap(),
            })
            .unwrap(),
        )
    }

    // Mock the state test suite
    fn mock_state() -> (StateVariableConfig, StateModel, Vec<StateVariable>) {
        let conf = StateVariableConfig::Distance {
            initial: Length::default(),
            accumulator: bool::default(),
            output_unit: Some(DistanceUnit::Miles),
        };

        let features = vec![
            ("edge_distance".to_string(), conf.clone()),
            ("edge_distance_1".to_string(), conf.clone()),
            ("edge_distance_2".to_string(), conf.clone()),
            ("edge_distance_3".to_string(), conf.clone()),
        ];
        let state_model = StateModel::new(features);

        // initialize states to 0
        let mut state = vec![StateVariable(0.0); 4];

        // create the state model components
        state_model
            .set_distance(
                &mut state,
                "edge_distance",
                &Length::new::<uom::si::length::mile>(1.0),
            )
            .unwrap();
        state_model
            .set_distance(
                &mut state,
                "edge_distance_1",
                &Length::new::<uom::si::length::mile>(2.0),
            )
            .unwrap();
        state_model
            .set_distance(
                &mut state,
                "edge_distance_2",
                &Length::new::<uom::si::length::mile>(3.0),
            )
            .unwrap();
        state_model
            .set_distance(
                &mut state,
                "edge_distance_3",
                &Length::new::<uom::si::length::mile>(4.0),
            )
            .unwrap();
        (conf, state_model, state)
    }

    // Mock the edge frontier context for insert_first op
    // used in both test_insert_first and test_invalid_depth_is_sentinel
    fn mock_edge_frontier_context<'a>(
        state_model: &StateModel,
        distance_val: f64,
        tree: &'a mut SearchTree,
        src_vertex: &'a Vertex,
        dst_vertex: &'a Vertex,
        mock_edge: &'a Edge,
        parent_label: &'a Label,
    ) -> EdgeFrontierContext<'a> {
        let label_0 = Label::Vertex(VertexId(0));
        let label_1 = Label::Vertex(VertexId(1));
        let label_2 = Label::Vertex(VertexId(2));

        // Define the traversal physics for each edge
        // 0 to 1
        let traversal_1 = EdgeTraversal {
            edge_list_id: EdgeListId(0),
            edge_id: EdgeId(1),
            cost: TraversalCost::empty(),
            result_state: vec![StateVariable(0.0); 4],
        };

        // 1 to 2
        let traversal_2 = EdgeTraversal {
            edge_list_id: EdgeListId(0),
            edge_id: EdgeId(2),
            cost: TraversalCost::empty(),
            result_state: vec![StateVariable(0.0); 4],
        };

        // relative to the context, traversal 3 will contains the "previous state."
        let mut previous_state = vec![StateVariable(0.0); 4];
        state_model
            .set_distance(
                &mut previous_state,
                "edge_distance",
                &Length::new::<uom::si::length::mile>(distance_val),
            )
            .unwrap();

        // 2 to 3
        let traversal_3 = EdgeTraversal {
            edge_list_id: EdgeListId(0),
            edge_id: EdgeId(3),
            cost: TraversalCost::empty(),
            result_state: previous_state,
        };

        // Create the topology in the search tree
        // 0 to 1
        tree.insert_trajectory(label_0, traversal_1, label_1.clone())
            .unwrap();
        // 1 to 2
        tree.insert_trajectory(label_1, traversal_2, label_2.clone())
            .unwrap();
        // 2 to 3
        tree.insert_trajectory(label_2, traversal_3, parent_label.clone())
            .unwrap();

        EdgeFrontierContext::new(parent_label, src_vertex, mock_edge, dst_vertex, tree)
    }

    /// **Tests the `shift` method of the `TripHistoryTraversalEngine`, ensuring that historical trip features are correctly updated in the state.**
    #[test]
    fn test_shift() {
        let trip_history_engine = mock_engine();

        let (conf, state_model, mut state) = mock_state();

        let src = &"edge_distance_2".to_string();
        let dst = &"edge_distance_3".to_string();

        trip_history_engine
            .shift(&mut state, &state_model, src, dst, &conf)
            .unwrap();

        assert_eq!(
            state_model.get_distance(&state, src).unwrap(),
            Length::new::<uom::si::length::mile>(3.0)
        );
        assert_eq!(
            state_model.get_distance(&state, dst).unwrap(),
            Length::new::<uom::si::length::mile>(3.0)
        );

        let src = &"edge_distance_1".to_string();
        let dst = &"edge_distance_2".to_string();
        trip_history_engine
            .shift(&mut state, &state_model, src, dst, &conf)
            .unwrap();

        assert_eq!(
            state_model.get_distance(&state, src).unwrap(),
            Length::new::<uom::si::length::mile>(2.0)
        );
        assert_eq!(
            state_model.get_distance(&state, dst).unwrap(),
            Length::new::<uom::si::length::mile>(2.0)
        )
    }

    /// **Tests the `insert_first` method of the `TripHistoryTraversalEngine`, ensuring that the first historical trip feature is correctly inserted into the state.**
    #[test]
    fn test_insert_first() {
        let trip_history_engine = mock_engine();
        let (conf, state_model, mut state) = mock_state();

        // The vertices and edge we are traversing. these are essentially dummy placeholders.
        let src_vertex = Vertex {
            vertex_id: VertexId(3), // this connects us to the previous edge.
            coordinate: InternalCoord(Coord { x: 0.0, y: 0.0 }),
        };
        let dst_vertex = Vertex {
            vertex_id: VertexId(4),
            coordinate: InternalCoord(Coord { x: 0.0, y: 0.0 }),
        };
        let mock_edge = Edge {
            edge_id: EdgeId(4),
            src_vertex_id: VertexId(3),
            dst_vertex_id: VertexId(4),
            distance: uom::si::f64::Length::default(),
            edge_list_id: EdgeListId(0),
        };
        let parent_label = Label::Vertex(VertexId(3));
        let mut tree = SearchTree::new_stateful(Direction::Forward);

        let ctx = mock_edge_frontier_context(
            &state_model,
            2.0, // Expected mock distance history
            &mut tree,
            &src_vertex,
            &dst_vertex,
            &mock_edge,
            &parent_label,
        );

        trip_history_engine
            .insert_first(&ctx, &mut state, &state_model, "edge_distance", &conf)
            .unwrap();

        // Verify that the current state's "edge_distance_1" now holds the "edge_distance" from the previous traversal
        let final_distance = state_model.get_distance(&state, "edge_distance_1").unwrap();
        assert_eq!(
            final_distance,
            uom::si::f64::Length::new::<uom::si::length::mile>(2.0)
        );
    }

    /// **Tests that sentinel values in the historical trip features are correctly shifted and maintained in the state.**
    ///
    /// This situation occurs when a search has not traversed far enough through the tree to be able to grab history features at a specific depth.
    ///
    /// For instance, consider the case where the desired history depth is 3, but we have only traversed 2 edges. In this case, using a value
    /// from depth 3 does not make sense, because there are no features at depth 3 in the history. The edge at a depth 3 backwards does not
    /// exist yet.
    ///
    /// Thus, the convention is to assign these features a sentinel value, and shift them appropriately as the search progresses.
    ///
    /// If the search is long enough, the sentinel values will be replaced by actual historical values.
    #[test]
    fn test_sentinel_shifting() {
        let trip_history_engine = mock_engine();

        let conf = StateVariableConfig::Distance {
            initial: Length::default(),
            accumulator: bool::default(),
            output_unit: Some(DistanceUnit::Miles),
        };
        let features = vec![
            ("edge_distance".to_string(), conf.clone()),
            ("edge_distance_1".to_string(), conf.clone()),
            ("edge_distance_2".to_string(), conf.clone()),
            ("edge_distance_3".to_string(), conf.clone()),
        ];
        let state_model = StateModel::new(features);

        let mut state = vec![StateVariable(0.0); 4];

        // At the start of search: history is initialized to sentinel values by the model
        // edge_distance is a f64, so it is NAN initially
        state_model
            .set_distance(
                &mut state,
                "edge_distance_1",
                &Length::new::<uom::si::length::mile>(f64::NAN),
            )
            .unwrap();
        state_model
            .set_distance(
                &mut state,
                "edge_distance_2",
                &Length::new::<uom::si::length::mile>(f64::NAN),
            )
            .unwrap();
        state_model
            .set_distance(
                &mut state,
                "edge_distance_3",
                &Length::new::<uom::si::length::mile>(f64::NAN),
            )
            .unwrap();

        // The vertices and edge we are traversing. these are essentially dummy placeholders.
        let src_vertex = Vertex {
            vertex_id: VertexId(3), // connects us to the previous edge.
            coordinate: InternalCoord(Coord { x: 0.0, y: 0.0 }),
        };
        let dst_vertex = Vertex {
            vertex_id: VertexId(4),
            coordinate: InternalCoord(Coord { x: 0.0, y: 0.0 }),
        };
        let mock_edge = Edge {
            edge_id: EdgeId(4),
            src_vertex_id: VertexId(3),
            dst_vertex_id: VertexId(4),
            distance: uom::si::f64::Length::default(),
            edge_list_id: EdgeListId(0),
        };
        let parent_label = Label::Vertex(VertexId(3));
        let mut tree = SearchTree::new_stateful(Direction::Forward);

        let ctx = mock_edge_frontier_context(
            &state_model,
            2.0, // Expected mock distance history
            &mut tree,
            &src_vertex,
            &dst_vertex,
            &mock_edge,
            &parent_label,
        );

        // update the trip history: shift -> insert_first
        trip_history_engine
            .update_history(&ctx, &mut state, &state_model)
            .unwrap();

        // Slot 1 should be populated with the valid history data (2.0)
        let d1 = state_model.get_distance(&state, "edge_distance_1").unwrap();
        assert_eq!(d1, Length::new::<uom::si::length::mile>(2.0));

        // Slots 2 and 3 should still be NAN because they shifted out of uninitialized NAN slots
        let d2 = state_model.get_distance(&state, "edge_distance_2").unwrap();
        assert!(d2.value.is_nan());

        let d3 = state_model.get_distance(&state, "edge_distance_3").unwrap();
        assert!(d3.value.is_nan());
    }
}
