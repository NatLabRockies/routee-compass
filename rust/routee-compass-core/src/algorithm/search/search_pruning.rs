use std::sync::Arc;

use crate::{
    algorithm::search::{EdgeTraversal, SearchTree, SearchTreeError},
    model::{
        label::{Label, LabelModel, LabelModelError},
        unit::Cost,
    },
};

/// Prune labels from the search tree that are Pareto-dominated by the new label.
///
/// A label is dominated if the new label is at least as good on all objectives
/// (cost and label state) and strictly better on at least one.
///
/// For label state comparison via `LabelModel::compare(prev, next)`:
/// - `Ordering::Less` means prev has lower (worse) state than next
/// - `Ordering::Equal` means states are equivalent  
/// - `Ordering::Greater` means prev has higher (better) state than next
///
/// We seek to maximize label state while minimizing cost.
pub fn prune_tree(
    tree: &mut SearchTree,
    next_label: &Label,
    traversal: &EdgeTraversal,
    label_model: Arc<dyn LabelModel>,
) -> Result<(), SearchTreeError> {
    if next_label.does_not_require_pruning() {
        return Ok(());
    }
    let next_cost = traversal.cost.objective_cost;
    let prev_entries = tree
        .get_labels_iter(*next_label.vertex_id())
        .map(|label| {
            let node = tree
                .get(&label)
                .ok_or_else(|| SearchTreeError::MissingNodeForLabel(label.clone()))?;
            let cost = node.traversal_cost().map(|tc| tc.objective_cost);
            Ok((label, cost.unwrap_or_default()))
        })
        .collect::<Result<Vec<_>, SearchTreeError>>()?;

    for (prev_label, prev_cost) in prev_entries.into_iter() {
        let remove = test_dominates(
            &prev_label,
            prev_cost,
            next_label,
            next_cost,
            label_model.clone(),
        )
        .map_err(|e| {
            SearchTreeError::PruningError(format!("label model comparison failed: {e}"))
        })?;
        if remove {
            // new label is pareto-dominant over this previous label.
            // we only remove the previous label if it is prunable (has no children)
            let prunable = tree
                .get(&prev_label)
                .map(|n| n.is_prunable())
                .unwrap_or_default();
            if prunable {
                let _ = tree.remove(&prev_label);
            }
        }
    }

    Ok(())
}

/// Test whether the next label Pareto-dominates the previous label.
///
/// Returns true if next dominates prev, meaning:
/// - next is at least as good on all objectives (cost and state)
/// - next is strictly better on at least one objective
///
/// Objectives:
/// - Maximize label state (higher is better)
/// - Minimize cost (lower is better)
fn test_dominates(
    prev_label: &Label,
    prev_cost: Cost,
    next_label: &Label,
    next_cost: Cost,
    label_model: Arc<dyn LabelModel>,
) -> Result<bool, LabelModelError> {
    let label_comparison = label_model.compare(prev_label, next_label)?;
    let dominates = match label_comparison {
        // prev < next: next has better (higher) state, so next dominates if cost is no worse
        std::cmp::Ordering::Less => next_cost <= prev_cost,
        // prev == next: states are equal, so next dominates only if strictly cheaper
        std::cmp::Ordering::Equal => next_cost < prev_cost,
        // prev > next: prev has better (higher) state, so next cannot dominate
        std::cmp::Ordering::Greater => false,
    };
    Ok(dominates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        label::Label,
        network::VertexId,
        state::{StateModel, StateVariable},
        unit::Cost,
    };

    #[test]
    fn test_not_pareto_dominated() {
        let label_model = build_soc_label_model();
        let prev_label = Label::VertexWithIntState {
            vertex_id: VertexId(0),
            state: 30,
        };
        let prev_cost = Cost::new(50.0);
        let next_label = Label::VertexWithIntState {
            vertex_id: VertexId(0),
            state: 80,
        };
        let next_cost = Cost::new(70.0);
        let is_dominated = test_dominates(
            &prev_label,
            prev_cost,
            &next_label,
            next_cost,
            label_model.clone(),
        )
        .expect("test invariant failed");
        assert!(!is_dominated);
    }

    #[test]
    fn test_is_pareto_dominated() {
        let label_model = build_soc_label_model();
        let prev_label = Label::VertexWithIntState {
            vertex_id: VertexId(0),
            state: 30,
        };
        let prev_cost = Cost::new(50.0);
        let next_label = Label::VertexWithIntState {
            vertex_id: VertexId(0),
            state: 80,
        };
        let next_cost = Cost::new(40.0);
        let is_dominated = test_dominates(
            &prev_label,
            prev_cost,
            &next_label,
            next_cost,
            label_model.clone(),
        )
        .expect("test invariant failed");
        assert!(is_dominated);
    }

    fn build_soc_label_model() -> Arc<dyn LabelModel> {
        struct SocLabelModel {}
        impl LabelModel for SocLabelModel {
            fn label_from_state(
                &self,
                _vertex_id: VertexId,
                _state: &[StateVariable],
                _state_model: &StateModel,
            ) -> Result<Label, LabelModelError> {
                unreachable!()
            }

            fn compare(
                &self,
                prev: &Label,
                next: &Label,
            ) -> Result<std::cmp::Ordering, LabelModelError> {
                match (prev, next) {
                    (
                        Label::VertexWithIntState { state: s1, .. },
                        Label::VertexWithIntState { state: s2, .. },
                    ) => Ok(s1.cmp(s2)),
                    _ => unreachable!(),
                }
            }
        }
        Arc::new(SocLabelModel {})
    }
}
