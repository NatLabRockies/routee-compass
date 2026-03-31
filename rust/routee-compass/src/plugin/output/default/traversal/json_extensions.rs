use crate::plugin::{input::InputField, output::OutputPluginError};
use std::fmt::Display;

pub enum TraversalJsonField {
    RouteOutput,
    GraphOutput,
}

impl TraversalJsonField {
    pub fn as_str(&self) -> &'static str {
        match self {
            TraversalJsonField::RouteOutput => "route",
            TraversalJsonField::GraphOutput => "graph",
        }
    }
}

impl Display for TraversalJsonField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

pub trait TraversalJsonExtensions {
    fn get_route_geometry_wkt(&self) -> Result<String, OutputPluginError>;
}

impl TraversalJsonExtensions for serde_json::Value {
    fn get_route_geometry_wkt(&self) -> Result<String, OutputPluginError> {
        let geometry = self
            .get(TraversalJsonField::RouteOutput.as_str())
            .ok_or_else(|| {
                OutputPluginError::MissingExpectedQueryField(InputField::Custom(
                    TraversalJsonField::RouteOutput.to_string(),
                ))
            })?
            .as_str()
            .ok_or_else(|| {
                OutputPluginError::QueryFieldHasInvalidType(
                    InputField::Custom(TraversalJsonField::RouteOutput.to_string()),
                    String::from("string"),
                )
            })?
            .to_string();
        Ok(geometry)
    }
}
