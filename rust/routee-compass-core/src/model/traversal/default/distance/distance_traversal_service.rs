use super::DistanceTraversalModel;
use crate::model::traversal::model::TraversalModel;
use crate::model::traversal::TraversalModelError;
use crate::model::traversal::TraversalModelService;
use crate::model::unit::DistanceUnit;
use std::sync::Arc;

pub struct DistanceTraversalService {
    pub distance_unit: DistanceUnit,
    pub include_trip_distance: bool,
}

impl DistanceTraversalService {
    pub fn new(
        distance_unit: DistanceUnit,
        include_trip_distance: bool,
    ) -> DistanceTraversalService {
        Self {
            distance_unit,
            include_trip_distance,
        }
    }
}

impl TraversalModelService for DistanceTraversalService {
    fn build(
        &self,
        _parameters: &serde_json::Value,
    ) -> Result<Arc<dyn TraversalModel>, TraversalModelError> {
        let m: Arc<dyn TraversalModel> = Arc::new(DistanceTraversalModel::new(
            self.distance_unit,
            self.include_trip_distance,
        ));
        Ok(m)
    }
}
