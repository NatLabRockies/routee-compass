use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;



/// sub-section of [`CompassAppConfig`] where the [`TraversalModelService`], [`AccessModelService`], and [`ConstraintModelService`] components
/// for an [`EdgeList`] are specified.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EdgeListSearchConfig {
    pub traversal: Value,
    pub constraint: Value,
}

impl JsonSchema for EdgeListSearchConfig {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "EdgeListSearchConfig".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        use routee_compass_core::model::constraint::default as C;
        use routee_compass_core::model::traversal::default as T;

  
        let default_traversal_models = vec![
            generator.subschema_for::<C::road_class::RoadClassConstraintConfig>(),
            generator.subschema_for::<C::turn_restrictions::TurnRestrictionConstraintConfig>(),
            generator.subschema_for::<C::vehicle_restrictions::VehicleRestrictionConfig>(),
        ];
        let default_constraint_models = vec![
            generator.subschema_for::<T::custom::CustomTraversalConfig>(),
            generator.subschema_for::<T::distance::DistanceTraversalConfig>(),
            generator.subschema_for::<T::grade::GradeConfiguration>(),
            generator.subschema_for::<T::speed::SpeedConfiguration>(),
            generator.subschema_for::<T::temperature::AmbientTemperatureConfig>(),
            generator.subschema_for::<T::time::TimeTraversalConfig>(),
            generator.subschema_for::<T::turn_delays::TurnDelayModelConfig>(),
        ];
        
        schemars::json_schema!({
            "search": {
                "constraint": {
                    "anyOf": default_constraint_models
                },
                "traversal": {
                    "anyOf": default_traversal_models
                }
            },
            "required": ["constraint", "traversal"],
            "$comment": "This schema shows default implementations. Users can extend by implementing the TraversalModel and ConstraintModel traits."
        })
    }
}