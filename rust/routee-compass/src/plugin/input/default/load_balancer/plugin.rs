use std::sync::Arc;

use super::weight_heuristic::WeightHeuristic;
use crate::app::search::SearchApp;
use crate::plugin::input::input_plugin::InputPlugin;
use crate::plugin::input::InputJsonExtensions;
use crate::plugin::input::InputPluginError;

pub struct LoadBalancerPlugin {
    pub heuristic: WeightHeuristic,
}

const NAME: &str = "load_balancer";

impl InputPlugin for LoadBalancerPlugin {
    fn process(
        &self,
        query: &mut serde_json::Value,
        _search_app: Arc<SearchApp>,
    ) -> Result<(), InputPluginError> {
        let w = self.heuristic.estimate_weight(query)?;
        let _updated = query.clone();
        query.add_query_weight_estimate(w)?;
        Ok(())
    }

    fn name(&self) -> &str {
        NAME
    }
}
