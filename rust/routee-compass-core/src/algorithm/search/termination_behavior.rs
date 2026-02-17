use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::algorithm::search::{SearchError, SearchResult};

#[derive(Clone, Debug, Serialize, Deserialize, Default, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TerminationFailurePolicy {
    /// treat any early-terminated search as a routing failure
    AllTerminationsFail,
    /// treat early-terminated path searches as failures. if the query
    /// has no destination (if the result is a tree), it is not a failure.
    /// used for reachability search, isochrone generation, etc.
    #[default]
    AllowTreeTermination,
}

impl TerminationFailurePolicy {
    /// validates and handles error propagation from search based on the [`TerminationFailurePolicy`].
    /// if this search result terminated early and this policy treats that result as an error, then
    /// return an error message, otherwise return nothing.
    pub fn handle_termination(
        &self,
        search_result: &SearchResult,
        has_destination: bool,
    ) -> Result<(), SearchError> {
        use TerminationFailurePolicy as T;
        if let Some(explanation) = &search_result.terminated {
            let error_on_terminate = match self {
                T::AllTerminationsFail => true,
                T::AllowTreeTermination if has_destination => true,
                _ => false,
            };
            if error_on_terminate {
                return Err(SearchError::QueryTerminated(explanation.clone()));
            }
        }
        Ok(())
    }
}
