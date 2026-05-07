use crate::{
    app::search::SearchApp,
    plugin::input::{input_plugin::InputPlugin, InputPluginError},
};
use indoc::formatdoc;
use log;
use std::sync::Arc;

pub struct DebugInputPlugin {}

const NAME: &str = "debug";

impl InputPlugin for DebugInputPlugin {
    fn name(&self) -> &str {
        NAME
    }

    fn process(
        &self,
        input: &mut serde_json::Value,
        _search_app: Arc<SearchApp>,
    ) -> Result<(), InputPluginError> {
        let json_result = serde_json::to_string_pretty(input);
        let string = match json_result {
            Ok(json_string) => json_string,
            Err(error) => {
                formatdoc! {r#"
                    {{
                        "message": "during debug plugin execution, failed to process incoming query as JSON",
                        "error": "{}"
                    }}
                "#, error}
            }
        };
        log::debug!("{string}");
        Ok(())
    }
}
