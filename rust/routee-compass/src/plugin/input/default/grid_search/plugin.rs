use crate::app::search::SearchApp;
use crate::plugin::input::input_field::InputField;
use crate::plugin::input::input_plugin::InputPlugin;
use crate::plugin::input::InputJsonExtensions;
use crate::plugin::input::InputPluginError;
use routee_compass_core::util::multiset::MultiSet;
use std::sync::Arc;

/// Builds an input plugin that duplicates queries if array-valued fields are present
/// by stepping through each combination of value
pub struct GridSearchPlugin {}

const NAME: &str = "grid_search";

impl InputPlugin for GridSearchPlugin {
    fn name(&self) -> &str {
        NAME
    }

    fn process(
        &self,
        input: &mut serde_json::Value,
        _search_app: Arc<SearchApp>,
    ) -> Result<(), InputPluginError> {
        match process_grid_search(input)? {
            None => Ok(()),
            Some(grid_expansion) => {
                let mut replacement = serde_json::json![grid_expansion];
                std::mem::swap(&mut replacement, input);
                Ok(())
            }
        }
    }
}

fn process_grid_search(
    input: &serde_json::Value,
) -> Result<Option<Vec<serde_json::Value>>, InputPluginError> {
    let grid_search_input = match input.get_grid_search() {
        Some(gsi) => gsi,
        None => return Ok(None),
    };

    // prevent recursion due to nested grid search keys
    let recurses = serde_json::to_string(grid_search_input)
        .map_err(|e| InputPluginError::JsonError { source: e })?
        .contains("grid_search");
    if recurses {
        return Err(InputPluginError::InputPluginFailed(String::from(
            "grid search section cannot contain the string 'grid_search'",
        )));
    }

    let map = grid_search_input
        .as_object()
        .ok_or_else(|| InputPluginError::UnexpectedQueryStructure(format!("{input:?}")))?;
    let mut keys: Vec<String> = vec![];
    let mut multiset_input: Vec<Vec<serde_json::Value>> = vec![];
    let mut multiset_indices: Vec<Vec<usize>> = vec![];
    for (k, v) in map {
        if let Some(v) = v.as_array() {
            keys.push(k.to_string());
            multiset_input.push(v.to_vec());
            let indices = (0..v.len()).collect();
            multiset_indices.push(indices);
        }
    }
    // for each combination, copy the grid search values into a fresh
    // copy of the source (minus the "grid_search" key)
    // let remove_key = InputField::GridSearch.to_str();
    let mut initial_map = input
        .as_object()
        .ok_or_else(|| InputPluginError::UnexpectedQueryStructure(format!("{input:?}")))?
        .clone();
    initial_map.remove(InputField::GridSearch.to_str());
    let initial = serde_json::json!(initial_map);
    let multiset = MultiSet::from(&multiset_indices);
    let result: Vec<serde_json::Value> = multiset
        .into_iter()
        .map(|combination| {
            let mut instance = initial.clone();
            let it = keys.iter().zip(combination.iter()).enumerate();
            for (set_idx, (key, val_idx)) in it {
                let value = multiset_input[set_idx][*val_idx].clone();
                match value {
                    serde_json::Value::Object(o) => {
                        for (k, v) in o.into_iter() {
                            instance[k] = v.clone();
                        }
                    }
                    _ => {
                        instance[key] = multiset_input[set_idx][*val_idx].clone();
                    }
                }
            }
            instance
        })
        .collect();

    Ok(Some(result))
}

#[cfg(test)]
mod test {
    use super::*;

    use serde_json::json;

    #[test]
    fn test_grid_search_empty_parent_object() {
        let input = serde_json::json!({
            "grid_search": {
                "bar": ["a", "b", "c"],
                "foo": [1.2, 3.4]
            }
        });

        let result = match process_grid_search(&input) {
            Ok(Some(rows)) => rows,
            Ok(None) => panic!("process_grid_search returned no expansions"),
            Err(e) => panic!("{}", e),
        };
        let expected = vec![
            json![{"bar":"a","foo":1.2}],
            json![{"bar":"b","foo":1.2}],
            json![{"bar":"c","foo":1.2}],
            json![{"bar":"a","foo":3.4}],
            json![{"bar":"b","foo":3.4}],
            json![{"bar":"c","foo":3.4}],
        ];
        assert_eq!(result, expected)
    }

    #[test]
    fn test_grid_search_persisted_parent_keys() {
        let input = serde_json::json!({
            "ignored_key": "ignored_value",
            "grid_search": {
                "bar": ["a", "b", "c"],
                "foo": [1.2, 3.4]
            }
        });

        let result = match process_grid_search(&input) {
            Ok(Some(rows)) => rows,
            Ok(None) => panic!("process_grid_search returned no expansions"),
            Err(e) => panic!("{}", e),
        };

        let expected = vec![
            json![{"bar":"a","foo":1.2,"ignored_key": "ignored_value"}],
            json![{"bar":"b","foo":1.2,"ignored_key": "ignored_value"}],
            json![{"bar":"c","foo":1.2,"ignored_key": "ignored_value"}],
            json![{"bar":"a","foo":3.4,"ignored_key": "ignored_value"}],
            json![{"bar":"b","foo":3.4,"ignored_key": "ignored_value"}],
            json![{"bar":"c","foo":3.4,"ignored_key": "ignored_value"}],
        ];

        assert_eq!(result, expected)
    }

    #[test]
    fn test_grid_search_using_objects() {
        let input = serde_json::json!({
            "ignored_key": "ignored_value",
            "grid_search": {
                "a": [1, 2],
                "ignored_inner_key": [
                    { "x": 0, "y": 0 },
                    { "x": 1, "y": 1 }
                ],
            }
        });

        let result = match process_grid_search(&input) {
            Ok(Some(rows)) => rows,
            Ok(None) => panic!("process_grid_search returned no expansions"),
            Err(e) => panic!("{}", e),
        };

        let expected = vec![
            json![{"a":1,"ignored_key":"ignored_value","x":0,"y":0}],
            json![{"a":2,"ignored_key":"ignored_value","x":0,"y":0}],
            json![{"a":1,"ignored_key":"ignored_value","x":1,"y":1}],
            json![ {"a":2,"ignored_key":"ignored_value","x":1,"y":1}],
        ];

        assert_eq!(result, expected)
    }

    #[test]
    fn test_nested() {
        let input = serde_json::json!({
            "abc": 123,
            "grid_search":{
                "model_name": ["2016_TOYOTA_Camry_4cyl_2WD","2017_CHEVROLET_Bolt"],
                "_ignore":[
                    { "name":"d1", "weights": { "distance":1, "time":0, "energy_electric":0 } },
                    { "name":"t1", "weights": { "distance":0, "time":1, "energy_electric":0 } },
                    { "name":"e1", "weights": { "distance":0, "time":0, "energy_electric":1 } }
                ]
            }
        });

        let result = match process_grid_search(&input) {
            Ok(Some(rows)) => rows,
            Ok(None) => panic!("process_grid_search returned no expansions"),
            Err(e) => panic!("{}", e),
        };

        let expected = vec![
            json![{"abc":123,"model_name":"2016_TOYOTA_Camry_4cyl_2WD","name":"d1","weights":{"distance":1,"time":0,"energy_electric":0}}],
            json![{"abc":123,"model_name":"2017_CHEVROLET_Bolt","name":"d1","weights":{"distance":1,"time":0,"energy_electric":0}}],
            json![{"abc":123,"model_name":"2016_TOYOTA_Camry_4cyl_2WD","name":"t1","weights":{"distance":0,"time":1,"energy_electric":0}}],
            json![{"abc":123,"model_name":"2017_CHEVROLET_Bolt","name":"t1","weights":{"distance":0,"time":1,"energy_electric":0}}],
            json![{"abc":123,"model_name":"2016_TOYOTA_Camry_4cyl_2WD","name":"e1","weights":{"distance":0,"time":0,"energy_electric":1}}],
            json![{"abc":123,"model_name":"2017_CHEVROLET_Bolt","name":"e1","weights":{"distance":0,"time":0,"energy_electric":1}}],
        ];

        assert_eq!(result, expected)
    }

    #[test]
    pub fn test_handle_recursion() {
        let input = serde_json::json!({
            "abc": 123,
            "grid_search":{
                "grid_search": {
                    "foo": [ "a", "b" ]
                }
            }
        });

        match process_grid_search(&input) {
            Ok(Some(_)) => panic!("process_grid_search should return an error"),
            Ok(None) => panic!("process_grid_search returned no error"),
            Err(_) => {}
        };
    }
}
