use super::CompassComponentError;
use crate::plugin::{
    input::{
        default::{
            debug::DebugInputPluginBuilder, grid_search::GridSearchBuilder,
            inject::InjectPluginBuilder, load_balancer::LoadBalancerBuilder,
        },
        InputPlugin, InputPluginBuilder,
    },
    output::{
        default::{
            summary::SummaryOutputPluginBuilder, traversal::TraversalPluginBuilder,
            uuid::UUIDOutputPluginBuilder,
        },
        OutputPlugin, OutputPluginBuilder,
    },
};
use inventory;
use itertools::Itertools;
use routee_compass_core::model::{
    constraint::{
        default::{
            combined::combined_builder::CombinedConstraintModelBuilder,
            no_restriction_builder::NoRestrictionBuilder,
            road_class::road_class_builder::RoadClassBuilder,
            turn_restrictions::turn_restriction_builder::TurnRestrictionBuilder,
            vehicle_restrictions::VehicleRestrictionBuilder,
        },
        ConstraintModelBuilder, ConstraintModelService,
    },
    label::{
        default::vertex_label_model::VertexLabelModelBuilder,
        label_model_builder::LabelModelBuilder, label_model_service::LabelModelService,
    },
    traversal::{
        default::{
            combined::CombinedTraversalBuilder, custom::CustomTraversalBuilder,
            elevation::ElevationTraversalBuilder, grade::GradeTraversalBuilder,
            temperature::TemperatureTraversalBuilder, time::TimeTraversalBuilder,
            turn_delays::TurnDelayTraversalModelBuilder,
        },
        TraversalModelBuilder, TraversalModelService,
    },
};
use routee_compass_core::{
    algorithm::map_matching::{
        HmmMapMatchingBuilder, LcssMapMatchingBuilder, MapMatchingAlgorithm, MapMatchingBuilder,
        SimpleMapMatchingBuilder,
    },
    config::{CompassConfigurationError, ConfigJsonExtensions},
    model::traversal::default::{distance::DistanceTraversalBuilder, speed::SpeedTraversalBuilder},
};
use routee_compass_powertrain::model::{
    charging::{
        battery::BatteryFilterBuilder, simple_charging_builder::SimpleChargingBuilder,
        soc_label_builder::SOCLabelModelBuilder,
    },
    EnergyModelBuilder,
};
use std::{collections::HashMap, rc::Rc, sync::Arc};

/// provides a plugin API for downstream libraries to inject values into the CompassBuilderInventory.
/// for details, see the [`inventory`] crate. must be a "type" defined in this crate in order to
/// get used at compile time, hence it's a struct.
pub struct BuilderRegistration(
    pub fn(&mut CompassBuilderInventory) -> Result<(), CompassConfigurationError>,
);
inventory::collect!(BuilderRegistration);

// this macro will register the default set of builders with inventory. these are iterated through in the CompassBuilderInventory::new method
// along with any plugins registered by downstream libraries.
inventory::submit! {
    BuilderRegistration(|builder| {
        builder.add_traversal_model("distance".to_string(),  Rc::new(DistanceTraversalBuilder {}));
        builder.add_traversal_model("speed".to_string(), Rc::new(SpeedTraversalBuilder {}));
        builder.add_traversal_model("time".to_string(), Rc::new(TimeTraversalBuilder {}));
        builder.add_traversal_model("grade".to_string(), Rc::new(GradeTraversalBuilder {}));
        builder.add_traversal_model("elevation".to_string(), Rc::new(ElevationTraversalBuilder {}));
        builder.add_traversal_model("energy".to_string(), Rc::new(EnergyModelBuilder {}));
        builder.add_traversal_model("simple_charging".to_string(), Rc::new(SimpleChargingBuilder::default()));
        builder.add_traversal_model("temperature".to_string(), Rc::new(TemperatureTraversalBuilder {}));
        builder.add_traversal_model("turn_delay".to_string(), Rc::new(TurnDelayTraversalModelBuilder {}));
        builder.add_traversal_model("custom".to_string(), Rc::new(CustomTraversalBuilder {}));
        builder.add_constraint_model("no_restriction".to_string(), Rc::new(NoRestrictionBuilder {}));
        builder.add_constraint_model("road_class".to_string(), Rc::new(RoadClassBuilder {}));
        builder.add_constraint_model("turn_restriction".to_string(), Rc::new(TurnRestrictionBuilder {}));
        builder.add_constraint_model("battery".to_string(), Rc::new(BatteryFilterBuilder::default()));
        builder.add_constraint_model("vehicle_restriction".to_string(), Rc::new(VehicleRestrictionBuilder {}));
        builder.add_label_model("vertex".to_string(), Rc::new(VertexLabelModelBuilder));
        builder.add_label_model("soc".to_string(), Rc::new(SOCLabelModelBuilder));
        builder.add_input_plugin("grid_search".to_string(), Rc::new(GridSearchBuilder {}));
        builder.add_input_plugin("load_balancer".to_string(), Rc::new(LoadBalancerBuilder {}));
        builder.add_input_plugin("inject".to_string(), Rc::new(InjectPluginBuilder {}));
        builder.add_input_plugin("debug".to_string(), Rc::new(DebugInputPluginBuilder {}));
        builder.add_output_plugin("traversal".to_string(), Rc::new(TraversalPluginBuilder {}));
        builder.add_output_plugin("summary".to_string(), Rc::new(SummaryOutputPluginBuilder {}));
        builder.add_output_plugin("uuid".to_string(), Rc::new(UUIDOutputPluginBuilder {}));
        builder.add_map_matching_model("simple".to_string(), Rc::new(SimpleMapMatchingBuilder {}));
        builder.add_map_matching_model("hmm".to_string(), Rc::new(HmmMapMatchingBuilder {}));
        builder.add_map_matching_model("lcss".to_string(), Rc::new(LcssMapMatchingBuilder {}));
        Ok(())
    })
}

/// Upstream component factory of [`crate::app::compass::compass_app::CompassApp`]
/// that builds components when constructing a CompassApp instance.
///
/// A [`CompassBuilderInventory`] instance is typically created via the [`CompassBuilderInventory::new']
/// method, which provides builders for commonly-used components.
/// Alternatively, there is a [`CompassBuilderInventory::new'] method to build an empty instance
/// Custom builder types can be added to an instance of CompassBuilderInventory and
/// then loaded into a CompassApp when the configuration TOML input provides a `type` argument
/// signaling the key associated with the builder below.
///
/// Builders (values in the hash maps) are simple structs that have empty constructors and
/// no fields, so any number of these may be present without resulting in any loading.
/// It is only once these are referenced during CompassApp construction that files and models
/// will be loaded and CPU/RAM impacted.
///
pub struct CompassBuilderInventory {
    traversal_model_builders: HashMap<String, Rc<dyn TraversalModelBuilder>>,
    constraint_model_builders: HashMap<String, Rc<dyn ConstraintModelBuilder>>,
    label_model_builders: HashMap<String, Rc<dyn LabelModelBuilder>>,
    input_plugin_builders: HashMap<String, Rc<dyn InputPluginBuilder>>,
    output_plugin_builders: HashMap<String, Rc<dyn OutputPluginBuilder>>,
    map_matching_builders: HashMap<String, Rc<dyn MapMatchingBuilder>>,
}

impl CompassBuilderInventory {
    /// Build an empty [`CompassBuilderInventory`] instance. does not inject any builders
    /// submitted by this or downstream libraries using [`inventory::submit!`].
    ///
    /// If no additional builders are added, this will be unable to create
    /// components for a [`crate::app::compass::compass_app::CompassApp`],
    /// so this method is only useful is seeking a blank slate for customizing.
    /// the [`CompassBuilderInventory::new`] method provides the default builder set and is
    /// the preferred method.
    ///
    /// # Returns
    ///
    /// * an instance of a CompassBuilderInventory that can be used to build a CompassApp
    pub fn empty() -> CompassBuilderInventory {
        CompassBuilderInventory {
            traversal_model_builders: HashMap::new(),
            constraint_model_builders: HashMap::new(),
            label_model_builders: HashMap::new(),
            input_plugin_builders: HashMap::new(),
            output_plugin_builders: HashMap::new(),
            map_matching_builders: HashMap::new(),
        }
    }

    /// creates a new [`CompassBuilderInventory`] with all registered builder objects injected from any [`inventory`] submissions.
    ///
    /// # Returns
    ///
    /// * an instance of a [`CompassBuilderInventory`] with all injected builders
    pub fn new() -> Result<CompassBuilderInventory, CompassConfigurationError> {
        let mut builder = Self::empty();

        // Iterate through all registered plugins
        for plugin_reg in inventory::iter::<BuilderRegistration> {
            (plugin_reg.0)(&mut builder)?;
        }
        Ok(builder)
    }

    pub fn add_traversal_model(&mut self, name: String, builder: Rc<dyn TraversalModelBuilder>) {
        let _ = self.traversal_model_builders.insert(name, builder);
    }

    pub fn add_constraint_model(&mut self, name: String, builder: Rc<dyn ConstraintModelBuilder>) {
        let _ = self.constraint_model_builders.insert(name, builder);
    }

    pub fn add_label_model(&mut self, name: String, builder: Rc<dyn LabelModelBuilder>) {
        let _ = self.label_model_builders.insert(name, builder);
    }

    pub fn add_input_plugin(&mut self, name: String, builder: Rc<dyn InputPluginBuilder>) {
        let _ = self.input_plugin_builders.insert(name, builder);
    }

    pub fn add_output_plugin(&mut self, name: String, builder: Rc<dyn OutputPluginBuilder>) {
        let _ = self.output_plugin_builders.insert(name, builder);
    }

    pub fn add_map_matching_model(&mut self, name: String, builder: Rc<dyn MapMatchingBuilder>) {
        let _ = self.map_matching_builders.insert(name, builder);
    }

    /// builds a traversal model with the specified type name with the provided
    /// traversal model configuration JSON
    pub fn build_traversal_model_service(
        &self,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModelService>, CompassConfigurationError> {
        // append the combined traversal model builder.
        let mut builders = self.traversal_model_builders.clone();
        builders.insert(
            String::from("combined"),
            Rc::new(CombinedTraversalBuilder::new(builders.clone())),
        );
        let tm_type = config.get_config_string(&"type", &"traversal")?;
        log::info!("loading traversal model service '{tm_type}'");
        let result = builders
            .get(&tm_type)
            .ok_or_else(|| {
                CompassConfigurationError::UnknownModelNameForComponent(
                    tm_type.clone(),
                    String::from("traversal"),
                    self.traversal_model_builders.keys().join(", "),
                )
            })
            .and_then(|b| {
                b.build(config)
                    .map_err(CompassConfigurationError::TraversalModelError)
            });
        result
    }

    /// builds a constraint model with the specified type name with the provided
    /// constraint model configuration JSON
    pub fn build_constraint_model_service(
        &self,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn ConstraintModelService>, CompassConfigurationError> {
        // append the combined constraint model builder.
        let mut builders = self.constraint_model_builders.clone();
        builders.insert(
            String::from("combined"),
            Rc::new(CombinedConstraintModelBuilder::new(builders.clone())),
        );
        let fm_type = config.get_config_string(&"type", &"constraint")?;
        log::info!("loading constraint model service '{fm_type}'");
        builders
            .get(&fm_type)
            .ok_or_else(|| {
                CompassConfigurationError::UnknownModelNameForComponent(
                    fm_type.clone(),
                    String::from("constraint"),
                    self.constraint_model_builders.keys().join(", "),
                )
            })
            .and_then(|b| {
                b.build(config)
                    .map_err(CompassConfigurationError::ConstraintModelError)
            })
    }

    pub fn build_label_model_service(
        &self,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn LabelModelService>, CompassConfigurationError> {
        let lm_type = config.get_config_string(&"type", &"label")?;
        log::info!("loading label model service '{lm_type}'");
        self.label_model_builders
            .get(&lm_type)
            .ok_or_else(|| {
                CompassConfigurationError::UnknownModelNameForComponent(
                    lm_type.clone(),
                    String::from("label"),
                    self.label_model_builders.keys().join(", "),
                )
            })
            .and_then(|b| {
                b.build(config)
                    .map_err(CompassConfigurationError::LabelModelError)
            })
    }

    pub fn build_input_plugins(
        &self,
        config: &[serde_json::Value],
    ) -> Result<Vec<Arc<dyn InputPlugin>>, CompassConfigurationError> {
        let mut plugins: Vec<Arc<dyn InputPlugin>> = Vec::new();
        for (idx, plugin_json) in config.iter().enumerate() {
            let plugin_type =
                plugin_json.get_config_string(&"type", &format!("input plugin {idx}"))?;
            log::info!("loading input plugin '{plugin_type}'");
            let builder = self
                .input_plugin_builders
                .get(&plugin_type)
                .ok_or_else(|| {
                    CompassConfigurationError::UnknownModelNameForComponent(
                        plugin_type.clone(),
                        String::from("Input Plugin"),
                        self.input_plugin_builders.keys().join(", "),
                    )
                })?;
            let input_plugin = builder.build(plugin_json)?;
            plugins.push(input_plugin);
        }
        Ok(plugins)
    }

    pub fn build_output_plugins(
        &self,
        config: &[serde_json::Value],
    ) -> Result<Vec<Arc<dyn OutputPlugin>>, CompassComponentError> {
        let mut plugins: Vec<Arc<dyn OutputPlugin>> = Vec::new();
        for (idx, plugin_json) in config.iter().enumerate() {
            let plugin_type =
                plugin_json.get_config_string(&"type", &format!("output_plugin {idx}"))?;
            log::info!("loading output plugin '{plugin_type}'");
            let builder = self
                .output_plugin_builders
                .get(&plugin_type)
                .ok_or_else(|| {
                    CompassConfigurationError::UnknownModelNameForComponent(
                        plugin_type.clone(),
                        String::from("Output Plugin"),
                        self.output_plugin_builders.keys().join(", "),
                    )
                })?;
            let output_plugin = builder.build(plugin_json)?;
            plugins.push(output_plugin);
        }
        Ok(plugins)
    }

    pub fn build_map_matching_algorithm(
        &self,
        config: &serde_json::Value,
    ) -> Result<Arc<dyn MapMatchingAlgorithm>, CompassConfigurationError> {
        let mm_type = config
            .get("type")
            .and_then(|t| t.as_str())
            .map(String::from)
            .unwrap_or_else(|| String::from("default"));
        log::info!("loading map matching algorithm '{mm_type}'");
        let builder = self.map_matching_builders.get(&mm_type).ok_or_else(|| {
            CompassConfigurationError::UnknownModelNameForComponent(
                mm_type.clone(),
                String::from("Map Matching Algorithm"),
                self.map_matching_builders.keys().join(", "),
            )
        })?;
        // Call build to create a configured instance from the builder
        builder
            .build(config)
            .map_err(CompassConfigurationError::MapMatchingError)
    }
}
