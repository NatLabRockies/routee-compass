use crate::model::{
    label::{Label, LabelModelError},
    network::VertexId,
    state::{StateModel, StateVariable},
};

/// Creates labels for vertices in a search algorithm that can be used to distinguish
/// between different states at the same vertex.
///
/// Labels are used in search algorithms to determine if two search states at the same
/// vertex are equivalent or if they represent different states that should be explored
/// separately. This is particularly important for algorithms that need to track
/// state-dependent information at vertices.
///
/// See the [`super::default`] module for implementations bundled with RouteE Compass:
///   - [VertexLabelModel]: creates simple vertex-based labels without state information
///
/// [VertexLabelModel]: super::default::vertex_label_model::VertexLabelModel
pub trait LabelModel: Send + Sync {
    /// Creates a label from the current search state at a given vertex.
    ///
    /// This method is called during search to generate a label that uniquely identifies
    /// the state at a vertex. The label is used to determine if two search states at
    /// the same vertex are equivalent and can be merged, or if they represent different
    /// states that should be explored separately.
    ///
    /// # Arguments
    ///
    /// * `vertex_id` - the vertex identifier for which to create a label
    /// * `state` - the current state vector at this vertex
    /// * `state_model` - provides access to the state vector components
    ///
    /// # Returns
    ///
    /// A [Label] that uniquely identifies this state at the vertex, or an error
    ///
    /// # Examples
    ///
    /// A simple vertex label model might return:
    /// ```ignore
    /// Ok(Label::Vertex(vertex_id))
    /// ```
    ///
    /// A state-dependent label model might return:
    /// ```ignore
    /// let battery_soc = state_model.get_energy(state, "trip_soc")?;
    /// Ok(Label::VertexWithIntState {
    ///     vertex_id,
    ///     state: vec![battery_soc.value() as u64],
    /// })
    /// ```
    fn label_from_state(
        &self,
        vertex_id: VertexId,
        state: &[StateVariable],
        state_model: &StateModel,
    ) -> Result<Label, LabelModelError>;

    /// Test whether a new label dominates over a previous label stored
    /// at the same vertex in a `SearchTree`.
    ///
    /// Uses [`std::cmp::Ordering`] with the following convention:
    ///   - [`Ordering::Greater`] — `next` label dominates `prev` (the new label is better)
    ///   - [`Ordering::Equal`] — `prev` and `next` labels are equivalent
    ///   - [`Ordering::Less`] — `prev` label dominates `next` (the existing label is better)
    ///
    /// This allows for pruning of the label state space during `SearchTree` insertion.
    ///
    /// # Arguments
    /// * `prev` - the existing label already stored at the vertex
    /// * `next` - the new label being considered for insertion
    ///
    /// # Result
    ///
    /// The comparison result between `prev` and `next` according to this label model's
    /// dominance rules, following the convention described above.
    fn compare(&self, prev: &Label, next: &Label) -> Result<std::cmp::Ordering, LabelModelError>;
}
