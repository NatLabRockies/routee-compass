use allocative::Allocative;
use serde::Serialize;

/// A state vector of usize values. provides a massive store of categorical 
/// values at the expense of inefficient memory usage. for a more memory-
/// efficient label vector, choose [super::U8StateVec].
#[derive(PartialEq, Eq, Hash, Debug, Clone, Serialize, Allocative)]
pub struct UsizeStateVec {
    state: Vec<usize>,
}

impl UsizeStateVec {
    /// Creates a new UsizeStateVec.
    pub fn new(state: Vec<usize>) -> Self {
        Self { state }
    }

    /// Gets a value from this vector.
    pub fn get(&self, index: usize) -> Option<usize> {
        self.state.get(index).cloned()
    }

    /// Length of this state vector.
    pub fn len(&self) -> usize {
        self.state.len()
    }

    /// Whether the state vector is empty.
    pub fn is_empty(&self) -> bool {
        self.state.is_empty()
    }

    /// Retrieve the state as a reference to a slice.
    pub fn as_slice(&self) -> &[usize] {
        &self.state
    }
}
