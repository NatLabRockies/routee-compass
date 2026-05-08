use super::{CoordinateOrientation, WriteMode};
use crate::{
    app::search::SearchApp,
    plugin::input::{input_plugin::InputPlugin, InputJsonExtensions, InputPluginError},
};
use routee_compass_core::util::geo::PolygonalRTree;
use serde_json::Value;
use std::sync::Arc;

pub enum InjectInputPlugin {
    Basic {
        key: String,
        value: serde_json::Value,
        write_mode: WriteMode,
    },
    Spatial {
        key: String,
        values: PolygonalRTree<f32, Value>,
        write_mode: WriteMode,
        orientation: CoordinateOrientation,
        default: Option<Value>,
    },
}

const BASIC_NAME: &str = "inject";

const SPATIAL_NAME: &str = "spatial_inject";

impl InputPlugin for InjectInputPlugin {
    fn process(
        &self,
        input: &mut serde_json::Value,
        _search_app: Arc<SearchApp>,
    ) -> Result<(), InputPluginError> {
        process_inject(self, input)
    }

    fn name(&self) -> &str {
        match self {
            InjectInputPlugin::Basic { .. } => BASIC_NAME,
            InjectInputPlugin::Spatial { .. } => SPATIAL_NAME,
        }
    }
}

pub fn process_inject(
    plugin: &InjectInputPlugin,
    input: &mut serde_json::Value,
) -> Result<(), InputPluginError> {
    match plugin {
        InjectInputPlugin::Basic {
            key,
            value,
            write_mode,
        } => write_mode.write_to_query(input, key, value),
        InjectInputPlugin::Spatial {
            values,
            key,
            write_mode,
            orientation,
            default,
        } => {
            let coord = match orientation {
                CoordinateOrientation::Origin => input.get_origin_coordinate(),
                CoordinateOrientation::Destination => match input.get_destination_coordinate() {
                    Ok(Some(coord)) => Ok(coord),
                    Ok(None) => Err(InputPluginError::InputPluginFailed(String::from(
                        "destination-oriented spatial inject plugin but query has no destination",
                    ))),
                    Err(e) => Err(e),
                },
            }?;
            let point = geo::Geometry::Point(geo::Point(coord));
            let mut intersect_iter = values.intersection(&point).map_err(|e| {
                InputPluginError::InputPluginFailed(format!(
                    "failure while intersecting spatial inject data: {e}"
                ))
            })?;
            match (intersect_iter.next(), default) {
                (None, None) => {
                    // nothing intersects + no default -> NOOP
                    Ok(())
                }
                (None, Some(default_value)) => {
                    // nothing intersects but we have a default
                    write_mode.write_to_query(input, key, default_value)
                }
                (Some(found), _) => {
                    // found an intersecting geometry with a value to assign
                    write_mode.write_to_query(input, key, &found.data)
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::{process_inject, InjectInputPlugin};
    use crate::plugin::input::default::inject::{
        inject_plugin_config::SpatialInjectPlugin, CoordinateOrientation, InjectPluginConfig,
        WriteMode,
    };
    use config::Config;
    use itertools::Itertools;
    use serde_json::{json, Value};
    use std::path::Path;

    #[test]
    fn test_kv() {
        let mut query = json!({});
        let key = String::from("key_on_query");
        let value = json![{"k": "v"}];
        let plugin = InjectInputPlugin::Basic {
            key: key.clone(),
            value: value.clone(),
            write_mode: WriteMode::Overwrite,
        };
        process_inject(&plugin, &mut query).expect("test failed");
        let result_value = query.get(&key).expect("test failed: key was not set");
        assert_eq!(
            result_value, &value,
            "test failed: value stored in GeoJSON with matching location does not match"
        )
    }

    #[test]
    fn test_kv_from_file() {
        let plugins = test_kv_conf();
        let result = plugins.iter().fold(json![{}], |mut input, plugin| {
            process_inject(plugin, &mut input).unwrap();
            input
        });
        let result_string = serde_json::to_string(&result).unwrap();
        let expected = String::from(
            r#"{"test_a":{"foo":"bar","baz":"bees"},"test_b":["test",5,3.14159],"test_c":[0,0,0,0]}"#,
        );
        assert_eq!(result_string, expected);
    }
    #[test]
    fn test_spatial_contains() {
        let mut query = json!({
            "origin_x": -105.11011135094863,
            "origin_y": 39.83906153425838
        });
        let source_key = Some(String::from("key_on_geojson"));
        let key = String::from("key_on_query");
        let plugin = setup_spatial(&source_key, &key, &None);
        process_inject(&plugin, &mut query).expect("test failed");
        let value = query.get(&key).expect("test failed: key was not set");
        let value_number = value.as_i64().expect("test failed: value was not a number");
        assert_eq!(
            value_number, 5000,
            "test failed: value stored in GeoJSON with matching location was not injected"
        )
    }

    #[test]
    fn test_spatial_not_contains() {
        let mut query = json!({
            "origin_x": -105.07021837975549,
            "origin_y": 39.93602243844981
        });
        let source_key = Some(String::from("key_on_geojson"));
        let key = String::from("key_on_query");
        let default = String::from("found default");
        let plugin = setup_spatial(&source_key, &key, &Some(json![default.clone()]));
        process_inject(&plugin, &mut query).expect("test failed");
        let value = query.get(&key).expect("test failed: key was not set");
        let value_str = value.as_str().expect("test failed: value was not a str");
        assert_eq!(
            value_str, &default,
            "test failed: value stored in GeoJSON with location that does not match did not return default fill value"
        )
    }

    #[test]
    fn test_spatial_not_contains_no_default() {
        let mut query = json!({
            "origin_x": -105.07021837975549,
            "origin_y": 39.93602243844981
        });
        let expected = query.clone();
        let source_key = Some(String::from("key_on_geojson"));
        let key = String::from("key_on_query");
        let plugin = setup_spatial(&source_key, &key, &None);
        process_inject(&plugin, &mut query).expect("test failed");
        assert_eq!(
            query, expected,
            "test failed: process should be idempotent when the query is not contained and there is no default value"
        )
    }

    #[test]
    fn test_spatial_from_json() {
        let source_key = String::from("key_on_geojson");
        let key = String::from("key_on_query");
        let filepath = if cfg!(target_os = "windows") {
            // Escape backslashes for Windows before adding to JSON
            test_geojson_filepath().replace("\\", "\\\\")
        } else {
            // Use the path as-is for non-Windows systems
            test_geojson_filepath()
        };
        let conf_str = format!(
            r#"
        {{
            "type": "inject",
            "format": "spatial_key_value",
            "spatial_input_file": "{}",
            "source_key": "{}",
            "key": "{}",
            "write_mode": "overwrite",
            "orientation": "origin"
        }}
        "#,
            filepath, &source_key, &key
        );
        let conf: InjectPluginConfig =
            serde_json::from_str(&conf_str).expect("failed to decode configuration");
        let plugin = conf.build().expect("failed to build plugin");
        let mut query = json!({
            "origin_x": -105.11011135094863,
            "origin_y": 39.83906153425838
        });

        process_inject(&plugin, &mut query).expect("failed to run plugin");
        let value = query.get(&key).expect("test failed: key was not set");
        let value_number = value.as_i64().expect("test failed: value was not a number");
        assert_eq!(
            value_number, 5000,
            "test failed: value stored in GeoJSON with matching location was not injected"
        )
    }

    fn test_geojson_filepath() -> String {
        let spatial_input_filepath = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("plugin")
            .join("input")
            .join("default")
            .join("inject")
            .join("test")
            .join("test.geojson");
        let path_str = spatial_input_filepath
            .to_str()
            .expect("test invariant failed: unable to convert filepath to string");
        path_str.to_string()
    }

    fn test_kv_conf() -> Vec<InjectInputPlugin> {
        let kv_conf_filepath = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("plugin")
            .join("input")
            .join("default")
            .join("inject")
            .join("test")
            .join("test_inject.toml");
        let conf_source = config::File::from(kv_conf_filepath);

        let config_toml = Config::builder()
            .add_source(conf_source)
            .build()
            .expect("test invariant failed");
        let config_json = config_toml
            .clone()
            .try_deserialize::<serde_json::Value>()
            .expect("test invariant failed");
        let input_plugin_array = config_json
            .get("input_plugin")
            .expect("TOML file should have an 'input_plugin' key")
            .clone();
        let array = input_plugin_array
            .as_array()
            .expect("key input_plugin should be an array");
        let plugins = array
            .iter()
            .map(|conf| {
                let ipc = serde_json::from_value::<InjectPluginConfig>(conf.clone())
                    .unwrap_or_else(|_| {
                        panic!(
                            "'input_plugin' entry should be valid: {}",
                            serde_json::to_string(&conf).unwrap_or_default()
                        )
                    });
                ipc.build().unwrap_or_else(|_| {
                    panic!(
                        "InjectPluginConfig.build failed: {}",
                        serde_json::to_string(&conf).unwrap_or_default()
                    )
                })
            })
            .collect_vec();
        plugins
    }

    fn setup_spatial(
        source_key: &Option<String>,
        key: &str,
        default: &Option<Value>,
    ) -> InjectInputPlugin {
        let spatial_input_file = test_geojson_filepath();
        let conf = InjectPluginConfig::SpatialKeyValue(SpatialInjectPlugin {
            spatial_input_file,
            source_key: source_key.clone(),
            key: key.to_owned(),
            write_mode: WriteMode::Overwrite,
            orientation: CoordinateOrientation::Origin,
            default: default.clone(),
        });

        conf.build().expect("test invariant failed")
    }
}
