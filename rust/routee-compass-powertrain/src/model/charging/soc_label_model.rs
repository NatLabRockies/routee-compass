use std::sync::Arc;

use routee_compass_core::model::{
    label::{
        label_enum::Label, label_model::LabelModel, label_model_error::LabelModelError,
        label_model_service::LabelModelService,
    },
    network::VertexId,
    state::{StateModel, StateVariable},
};

pub struct SOCLabelModel {
    pub soc_percent_bins: Vec<u64>,
}

impl SOCLabelModel {
    pub fn new(soc_percent_bins: Vec<u64>) -> Self {
        SOCLabelModel { soc_percent_bins }
    }
    pub fn from_range(start: u64, end: u64, step: u64) -> Self {
        let soc_percent_bins = (start..=end).step_by(step as usize).collect();
        SOCLabelModel { soc_percent_bins }
    }
}

impl LabelModel for SOCLabelModel {
    fn label_from_state(
        &self,
        vertex_id: VertexId,
        state: &[StateVariable],
        state_model: &StateModel,
    ) -> Result<Label, LabelModelError> {
        let soc = state_model.get_ratio(state, "trip_soc")?;
        let soc_percent = soc.get::<uom::si::ratio::percent>();
        let soc_bin = self
            .soc_percent_bins
            .iter()
            .position(|&bin| bin as f64 >= soc_percent)
            .unwrap_or(self.soc_percent_bins.len() - 1);

        Ok(Label::VertexWithIntState {
            vertex_id,
            state: soc_bin,
        })
    }
}

impl LabelModelService for SOCLabelModel {
    fn build(
        &self,
        _query: &serde_json::Value,
        _state_model: Arc<StateModel>,
    ) -> Result<Arc<dyn LabelModel>, LabelModelError> {
        let model = SOCLabelModel {
            soc_percent_bins: self.soc_percent_bins.clone(),
        };
        Ok(Arc::new(model))
    }
}
