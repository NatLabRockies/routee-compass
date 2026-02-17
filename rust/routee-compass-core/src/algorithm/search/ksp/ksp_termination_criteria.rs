use std::fmt::Display;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone, Debug, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum KspTerminationCriteria {
    /// for a given k-shortest paths search, run exactly k iterations
    #[default]
    Exact,
    /// for a given k-shortest paths search, run up to $max solutions found when possible.
    MaxIteration { max: u64 },
    /// for a given k-shortest path search, run k * $factor solutions found when possible
    /// factor * k should probably be greater than k (factor should be greater than 1).
    Factor { factor: u64 },
}

impl Display for KspTerminationCriteria {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KspTerminationCriteria::Exact => write!(f, "terminate with up to k routes found"),
            KspTerminationCriteria::MaxIteration { max } => {
                write!(f, "terminate with {max} routes found")
            }
            KspTerminationCriteria::Factor { factor } => {
                write!(f, "terminate with k*{factor} routes found")
            }
        }
    }
}

impl KspTerminationCriteria {
    /// test if ksp should terminate.
    /// this user-configurable interface for testing algorithm termination
    /// are run directly before popping the next value from the search frontier queue.
    /// these are not self-validating; if the user has provided inputs that are invalid,
    /// no error will occur. invalid arguments:
    /// - for MaxIteration, a max value less than k
    /// - for Factor, a factor less than or equal to 1
    ///
    /// # Arguments
    ///
    /// * `k` - the user-provided number of paths requested
    /// * `solution_size` - at the current search iteration, the size of the solution collection
    pub fn terminate_search(&self, k: usize, solution_size: usize) -> bool {
        match self {
            KspTerminationCriteria::Exact => solution_size == k,
            KspTerminationCriteria::MaxIteration { max } => {
                KspTerminationCriteria::Exact.terminate_search(k, solution_size)
                    && *max as usize >= k
            }
            KspTerminationCriteria::Factor { factor } => {
                KspTerminationCriteria::Exact.terminate_search(k, solution_size)
                    && (*factor as usize * solution_size) >= k
            }
        }
    }
}
