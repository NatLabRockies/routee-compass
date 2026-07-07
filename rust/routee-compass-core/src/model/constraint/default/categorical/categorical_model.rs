use crate::model::constraint::ConstraintModel;
struct CategoricalConstraintModel {}

impl ConstraintModel for CategoricalConstraintModel {
    fn valid_frontier(
        &self,
        ctx: &crate::model::traversal::EdgeFrontierContext,
        state: &[crate::model::state::StateVariable],
        state_model: &crate::model::state::StateModel,
    ) -> Result<bool, crate::model::constraint::ConstraintModelError> {
        todo!();
    }
    fn valid_edge(
        &self,
        edge: &crate::model::network::Edge,
    ) -> Result<bool, crate::model::constraint::ConstraintModelError> {
        todo!();
    }
}
