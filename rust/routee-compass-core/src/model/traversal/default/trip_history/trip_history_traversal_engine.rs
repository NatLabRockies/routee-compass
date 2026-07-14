use super::TripHistoryConfig;

use crate::model::traversal::TraversalModelError;

pub struct TripHistoryEngine {
    config: TripHistoryConfig,
}

impl TryFrom<TripHistoryConfig> for TripHistoryEngine {
    type Error = TraversalModelError;

    fn try_from(config: TripHistoryConfig) -> Result<Self, Self::Error> {
        Ok(Self { config })
    }
}
