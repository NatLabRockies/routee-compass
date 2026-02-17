use schemars::{json_schema, JsonSchema};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// sub-section of [`CompassAppConfig`] where the [`TraversalModelService`] and [`ConstraintModelService`] components
/// for an [`EdgeList`] are specified.
#[derive(Serialize, Deserialize, Clone, Debug, JsonSchema)]
pub struct EdgeListSearchConfig {
    pub traversal: TraversalConfig,
    pub constraint: ConstraintConfig,
}

impl EdgeListSearchConfig {
    pub fn get_traversal_config(&self) -> &Value {
        &self.traversal.0
    }

    pub fn get_constraint_config(&self) -> &Value {
        &self.constraint.0
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TraversalConfig(pub Value);

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConstraintConfig(pub Value);

impl JsonSchema for TraversalConfig {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "TraversalConfig".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        use routee_compass_core::model::traversal::default as T;

        let default_models = vec![
            generator.subschema_for::<T::custom::CustomTraversalConfig>(),
            generator.subschema_for::<T::distance::DistanceTraversalConfig>(),
            generator.subschema_for::<T::grade::GradeConfiguration>(),
            generator.subschema_for::<T::speed::SpeedConfiguration>(),
            generator.subschema_for::<T::temperature::AmbientTemperatureConfig>(),
            generator.subschema_for::<T::time::TimeTraversalConfig>(),
            generator.subschema_for::<T::turn_delays::TurnDelayModelConfig>(),
        ];

        json_schema!({
            "anyOf": default_models,
            "$comment": "This schema shows default implementations. Users can extend by implementing the TraversalModel trait."
        })
    }
}

impl JsonSchema for ConstraintConfig {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ConstraintConfig".into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        use routee_compass_core::model::constraint::default as C;

        let default_models = vec![
            generator.subschema_for::<C::road_class::RoadClassConstraintConfig>(),
            generator.subschema_for::<C::turn_restrictions::TurnRestrictionConstraintConfig>(),
            generator.subschema_for::<C::vehicle_restrictions::VehicleRestrictionConfig>(),
        ];

        json_schema!({
            "anyOf": default_models,
            "$comment": "This schema shows default implementations. Users can extend by implementing the ConstraintModel trait."
        })
    }
}
