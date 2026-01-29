use std::fs;
use std::path::Path;

use indoc::formatdoc;
use serde::{Deserialize, Serialize};

/// optional extensions to the traversal model generator
#[derive(Serialize, Deserialize, Debug, Clone, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum TraversalExtensions {
    /// include the config.rs and params.rs files and deserialize the inputs to 
    /// builder and service .build() methods into these types.
    TypedConfig,
    /// also include an engine.rs file for module business logic with a TryFrom<&Config>
    /// implementation stub.
    TypedConfigAndEngine
}

/// creates the file contents and writes to the files with template code.
pub fn generate_traversal_module(
    pascal_case_name: &str,
    snake_case_name: &str,
    path: &Path,
    extensions: Option<&TraversalExtensions>
) -> Result<(), Box<dyn std::error::Error>> {
    let module_dir = path.join(snake_case_name);
    fs::create_dir_all(&module_dir)?;

    let typed_config = extensions.is_some();
    let engine = matches!(extensions, Some(&TraversalExtensions::TypedConfigAndEngine));

    // Generate files with template content
    fs::write(
        module_dir.join("mod.rs"),
        mod_template(pascal_case_name, typed_config, engine),
    )?;
    fs::write(
        module_dir.join("model.rs"),
        model_template(pascal_case_name, extensions),
    )?;
    match extensions {
        None => {
            fs::write(
                module_dir.join("builder.rs"),
                builder_template(pascal_case_name),
            )?;
            fs::write(
                module_dir.join("service.rs"),
                service_template(pascal_case_name),
            )?;
        },
        Some(&TraversalExtensions::TypedConfig) => {
            fs::write(
                module_dir.join("builder.rs"),
                builder_template_typed(pascal_case_name),
            )?;
            fs::write(
                module_dir.join("service.rs"),
                service_template_typed(pascal_case_name),
            )?;
            fs::write(
                module_dir.join("config.rs"),
                config_template(pascal_case_name),
            )?;
            fs::write(
                module_dir.join("params.rs"),
                params_template(pascal_case_name),
            )?; 
        },
        Some(&TraversalExtensions::TypedConfigAndEngine) => {
            fs::write(
                module_dir.join("builder.rs"),
                builder_template_engine(pascal_case_name),
            )?;
            fs::write(
                module_dir.join("service.rs"),
                service_template_engine(pascal_case_name),
            )?;
            fs::write(
                module_dir.join("config.rs"),
                config_template(pascal_case_name),
            )?;
            fs::write(
                module_dir.join("params.rs"),
                params_template(pascal_case_name),
            )?; 
            fs::write(
                module_dir.join("engine.rs"),
                engine_template(pascal_case_name),
            )?; 
        }
    }

    println!(
        "✓ Generated TraversalModel module at {}/{}",
        path.display(),
        snake_case_name
    );
    println!("  Next steps:");
    println!("  1. Add 'mod {};' to your lib.rs", snake_case_name);
    println!("  2. Implement the trait methods in each file");
    println!(
        "  3. Register builder with inventory::submit! in your plugin registration"
    );

    Ok(())
}

/// generates the mod.rs file content for a new traversal model
pub fn mod_template(pascal_case_name: &str, typed_config: bool, engine: bool) -> String {
    // the basic set of files, optionally extended with other add-ons
    let mut entries = vec!["builder", "service", "model"];
    if typed_config {
        entries.push("config");
        entries.push("params");
    }
    if engine {
        entries.push("engine");
    }
    entries.sort();

    let mut result = String::new();

    // import each file as a module (not pub)
    for entry in entries.iter() {
        let mod_row = format!("mod {entry};\n");
        result.push_str(&mod_row);
    }
    result.push_str("\n");

    // expose each type from each file (pub)
    for entry in entries.iter() {
        let mut entry_cap = entry.to_string();
        if let Some(first_char) = entry_cap.get_mut(0..1) {
            first_char.make_ascii_uppercase();
        }
        let use_row = format!("pub use {entry}::{pascal_case_name}{entry_cap};\n");
        result.push_str(&use_row);
    }

    result
}

pub fn builder_template(pascal_case_name: &str) -> String {
    let service_name = format!("{pascal_case_name}Service");
    let builder_name = format!("{pascal_case_name}Builder");
    formatdoc!("
        use std::sync::Arc;

        use super::{service_name};

        use routee_compass_core::model::traversal::{{TraversalModelBuilder, TraversalModelError, TraversalModelService}};

        pub struct {builder_name} {{}}

        impl TraversalModelBuilder for {builder_name} {{
            fn build(
                &self,
                _params: &serde_json::Value,
            ) -> Result<Arc<dyn TraversalModelService>, TraversalModelError> {{
                let service = {service_name}::new();
                Ok(Arc::new(service))
            }}
        }}
    ")
}

pub fn builder_template_typed(pascal_case_name: &str) -> String {
    let builder_name = format!("{pascal_case_name}Builder");
    let service_name = format!("{pascal_case_name}Service");
    let config_name = format!("{pascal_case_name}Config");
    formatdoc!("
        use std::sync::Arc;

        use super::{{{config_name}, {service_name}}};

        use routee_compass_core::model::traversal::{{
            TraversalModelBuilder, 
            TraversalModelError, 
            TraversalModelService
        }};

        pub struct {builder_name} {{}}

        impl TraversalModelBuilder for {builder_name} {{
            fn build(
                &self,
                value: &serde_json::Value,
            ) -> Result<Arc<dyn TraversalModelService>, TraversalModelError> {{
                let config: {config_name} = serde_json::from_value(value.clone())
                    .map_err(|e| {{
                        let msg = format!(\"failure reading params for {pascal_case_name} service: {{e}}\");
                        TraversalModelError::BuildError(msg)
                    }})?;
                let service = {service_name}::new(config);
                Ok(Arc::new(service))
            }}
        }}
    ")
}

pub fn builder_template_engine(pascal_case_name: &str) -> String {
    let builder_name = format!("{pascal_case_name}Builder");
    let service_name = format!("{pascal_case_name}Service");
    let config_name = format!("{pascal_case_name}Config");
    let engine_name = format!("{pascal_case_name}Engine");

    formatdoc!("
        use std::sync::Arc;

        use super::{{{config_name}, {engine_name}, {service_name}}};

        use routee_compass_core::model::traversal::{{
            TraversalModelBuilder, 
            TraversalModelError, 
            TraversalModelService
        }};

        pub struct {builder_name} {{}}

        impl TraversalModelBuilder for {builder_name} {{
            fn build(
                &self,
                config: &serde_json::Value,
            ) -> Result<Arc<dyn TraversalModelService>, TraversalModelError> {{
                let config: {config_name} = serde_json::from_value(config.clone())
                    .map_err(|e| {{
                        let msg = format!(\"failure reading config for {pascal_case_name} builder: {{e}}\");
                        TraversalModelError::BuildError(msg)
                    }})?;
                let engine = {engine_name}::try_from(config)
                    .map_err(|e| {{
                        let msg = format!(\"failure building engine from config for {pascal_case_name} builder: {{e}}\");
                        TraversalModelError::BuildError(msg)
                    }})?;
                let service = {service_name}::new(engine);
                Ok(Arc::new(service))
            }}
        }}
    ")
}
pub fn service_template(pascal_case_name: &str) -> String {
    let service_name = format!("{pascal_case_name}Service");
    let model_name = format!("{pascal_case_name}Model");
    formatdoc!("
        use std::sync::Arc;

        use super::{model_name};

        use routee_compass_core::model::traversal::{{TraversalModel, TraversalModelError, TraversalModelService}};

        pub struct {service_name} {{}}

        impl TraversalModelService for {service_name} {{
            fn build(
                &self,
                _query: &serde_json::Value,
            ) -> Result<Arc<dyn TraversalModel>, TraversalModelError> {{
                let model = {model_name}::new();
                Ok(Arc::new(model))
            }}
        }}

        impl {service_name} {{
            pub fn new() -> Self {{
                Self {{}}
            }}
        }}
    ")
}

pub fn service_template_typed(pascal_case_name: &str) -> String {
    let service_name = format!("{pascal_case_name}Service");
    let config_name = format!("{pascal_case_name}Config");
    let params_name = format!("{pascal_case_name}Params");
    let model_name = format!("{pascal_case_name}Model");
    formatdoc!("
        use std::sync::Arc;

        use super::{{{config_name}, {params_name}, {model_name}}};

        use routee_compass_core::model::traversal::{{TraversalModel, TraversalModelError, TraversalModelService}};

        pub struct {service_name} {{
            config: Arc<{config_name}>
        }}

        impl {service_name} {{
            pub fn new(config: {config_name}) -> Self {{
                Self {{
                    config: Arc::new(config)
                }}
            }}
        }}

        impl TraversalModelService for {service_name} {{
            fn build(
                &self,
                query: &serde_json::Value,
            ) -> Result<Arc<dyn TraversalModel>, TraversalModelError> {{
                let params: {params_name} = serde_json::from_value(query.clone())
                    .map_err(|e| {{
                        let msg = format!(\"failure reading params for {pascal_case_name} service: {{e}}\");
                        TraversalModelError::BuildError(msg)
                    }})?;
                let model = {model_name}::new(self.config.clone(), params);
                Ok(Arc::new(model))
            }}
        }}
    ")
}

pub fn service_template_engine(pascal_case_name: &str) -> String {
    let service_name = format!("{pascal_case_name}Service");
    let engine_name = format!("{pascal_case_name}Engine");
    let params_name = format!("{pascal_case_name}Params");
    let model_name = format!("{pascal_case_name}Model");
    formatdoc!("
        use std::sync::Arc;

        use super::{{{engine_name}, {params_name}, {model_name}}};

        use routee_compass_core::model::traversal::{{TraversalModel, TraversalModelError, TraversalModelService}};

        pub struct {service_name} {{
            engine: Arc<{engine_name}>
        }}

        impl {service_name} {{
            pub fn new(engine: {engine_name}) -> Self {{
                Self {{
                    engine: Arc::new(engine)
                }}
            }}
        }}

        impl TraversalModelService for {service_name} {{
            fn build(
                &self,
                query: &serde_json::Value,
            ) -> Result<Arc<dyn TraversalModel>, TraversalModelError> {{
                let params: {params_name} = serde_json::from_value(query.clone())
                    .map_err(|e| {{
                        let msg = format!(\"failure reading params for {pascal_case_name} service: {{e}}\");
                        TraversalModelError::BuildError(msg)
                    }})?;
                let model = {model_name}::new(self.engine.clone(), params);
                Ok(Arc::new(model))
            }}
        }}
    ")
}

pub fn model_template(pascal_case_name: &str, extensions: Option<&TraversalExtensions>) -> String {
    let model_name = format!("{pascal_case_name}Model");
    let config_name = format!("{pascal_case_name}Config");
    let engine_name = format!("{pascal_case_name}Engine");
    let params_name = format!("{pascal_case_name}Params");

    // 
    let super_import = match extensions {
        None => "".to_string(),
        Some(TraversalExtensions::TypedConfig) => format!("use super::{{{config_name}, {params_name}}};"),
        Some(TraversalExtensions::TypedConfigAndEngine) => format!("use super::{{{engine_name}, {params_name}}};"),
    };

    let struct_def = match extensions {
        None => formatdoc!("
            pub struct {model_name} {{}}

            impl {model_name} {{
                pub fn new() -> Self {{
                    Self {{}}
                }}
            }}
        "),
        Some(TraversalExtensions::TypedConfig) => formatdoc!("
            pub struct {model_name} {{
                pub config: Arc<{config_name}>,
                pub params: {params_name}
            }}

            impl {model_name} {{
                pub fn new(config: Arc<{config_name}>, params: {params_name}) -> Self {{
                    // modify this and the struct definition if additional pre-processing
                    // is required during model instantiation from query parameters.
                    Self {{
                        config, params
                    }}
                }}
            }}
        "),
        Some(TraversalExtensions::TypedConfigAndEngine) => formatdoc!("
            pub struct {model_name} {{
                pub engine: Arc<{engine_name}>,
                pub params: {params_name}
            }}

            impl {model_name} {{
                pub fn new(engine: Arc<{engine_name}>, params: {params_name}) -> Self {{
                    // modify this and the struct definition if additional pre-processing
                    // is required during model instantiation from query parameters.
                    Self {{
                        engine, params
                    }}
                }}
            }}
        ")
    };

    formatdoc!("
        use std::sync::Arc;

        {super_import}

        use routee_compass_core::{{
            algorithm::search::SearchTree,
            model::{{
                network::{{Edge, Vertex}},
                state::{{InputFeature, StateModel, StateVariable, StateVariableConfig}},
                traversal::{{TraversalModel, TraversalModelError}},
            }},
        }};
    
        {struct_def}

        impl TraversalModel for {model_name} {{
            fn name(&self) -> String {{
                \"{model_name}\".to_string()
            }}

            fn input_features(&self) -> Vec<InputFeature> {{
                todo!()
            }}

            fn output_features(&self) -> Vec<(String, StateVariableConfig)> {{
                todo!()
            }}

            fn traverse_edge(
                &self,
                _trajectory: (&Vertex, &Edge, &Vertex),
                _state: &mut Vec<StateVariable>,
                _tree: &SearchTree,
                _state_model: &StateModel,
            ) -> Result<(), TraversalModelError> {{
                todo!()
            }}

            fn estimate_traversal(
                &self,
                _od: (&Vertex, &Vertex),
                _state: &mut Vec<StateVariable>,
                _tree: &SearchTree,
                _state_model: &StateModel,
            ) -> Result<(), TraversalModelError> {{
                todo!()
            }}
        }}
    ")
}

pub fn config_template(pascal_case_name: &str) -> String {
    let config_name = format!("{pascal_case_name}Config");
    formatdoc!("
        use serde::{{Deserialize, Serialize}};

        #[derive(Deserialize, Serialize, Clone, Debug)]
        pub struct {config_name} {{}}
    ")
}

pub fn params_template(pascal_case_name: &str) -> String {
    let params_name = format!("{pascal_case_name}Params");
    formatdoc!("
        use serde::{{Deserialize, Serialize}};

        #[derive(Deserialize, Serialize, Clone, Debug)]
        pub struct {params_name} {{}}
    ")
}

pub fn engine_template(pascal_case_name: &str) -> String {
    let engine_name = format!("{pascal_case_name}Engine");
    let config_name = format!("{pascal_case_name}Config");
    formatdoc!("
        use super::{config_name};

        use routee_compass_core::model::traversal::TraversalModelError;

        pub struct {engine_name} {{}}

        impl TryFrom<{config_name}> for {engine_name} {{
            type Error = TraversalModelError;

            fn try_from(_config: {config_name}) -> Result<Self, Self::Error> {{
                todo!()
            }}
        }}
    ")
}