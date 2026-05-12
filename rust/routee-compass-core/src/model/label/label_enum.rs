use std::{fmt::Display, hash::Hash};

use allocative::Allocative;
use serde::Serialize;

use super::{U8StateVec, UsizeStateVec};
use crate::model::{label::label_model_error::LabelModelError, network::VertexId};

/// The required length for OS-aligned state vectors.
/// This is the word size of the target architecture.
#[cfg(target_pointer_width = "32")]
pub const OS_ALIGNED_STATE_LEN: usize = 4;

#[cfg(target_pointer_width = "64")]
pub const OS_ALIGNED_STATE_LEN: usize = 8;

#[derive(PartialEq, Eq, Hash, Debug, Clone, Serialize, Allocative)]
pub enum Label {
    Vertex(VertexId),
    VertexWithIntState {
        vertex_id: VertexId,
        state: usize,
    },
    VertexWithUsizeStateVec {
        vertex_id: VertexId,
        state: Box<UsizeStateVec>,
    },
    /// Store u8 state data. more efficient memory layout for smaller
    /// numbers or categorical data with 256 or fewer categories.
    ///
    /// The state vector is stored on the heap to keep the enum size small.
    ///
    /// For memory alignment, the Vec<u8> will be extended to the
    /// nearest integer multiple of OS_ALIGNED_STATE_LEN that covers
    /// the provided state values.
    ///
    /// In order to ensure reading the state value produces a slice
    /// of the same length as the Vec<u8> used to construct this
    /// Label, we also store a state_len: u8 value. This limits to
    /// state sizes up to 256 elements. This is guaranteed when using
    /// the get_u8_state method for retrieval.
    VertexWithU8StateVec {
        vertex_id: VertexId,
        state: Box<U8StateVec>,
    },
}

impl Label {
    /// Creates a new VertexWithIntState.
    ///
    /// # Arguments
    ///
    /// * `vertex_id` - The vertex identifier
    /// * `state` - The integer state value
    pub fn new_int_state(vertex_id: VertexId, state: usize) -> Self {
        Label::VertexWithIntState { vertex_id, state }
    }

    /// Creates a new VertexWithUsizeStateVec.
    ///
    /// # Arguments
    ///
    /// * `vertex_id` - The vertex identifier
    /// * `state` - The integer state vector
    pub fn new_usize_state_vec(vertex_id: VertexId, state: Vec<usize>) -> Self {
        Label::VertexWithUsizeStateVec {
            vertex_id,
            state: Box::new(UsizeStateVec::new(state)),
        }
    }

    /// Creates a new VertexWithU8StateVec with validation.
    ///
    /// # Arguments
    ///
    /// * `vertex_id` - The vertex identifier
    /// * `state` - The u8 state vector which can be up to 255 values long.
    ///
    /// # Returns
    ///
    /// A Result containing the Label or a LabelModelError if the state vector length exceeds u8::MAX.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use routee_compass_core::model::label::label_enum::{Label, OS_ALIGNED_STATE_LEN};
    /// # use routee_compass_core::model::network::VertexId;
    /// let vertex_id = VertexId(42);
    /// let state = vec![1u8, 2u8, 3u8, 4u8];
    /// let label = Label::new_u8_state(vertex_id, &state).unwrap();
    /// let out_state = label.get_u8_state().unwrap();
    /// assert_eq!(state.as_slice(), out_state);
    /// ```
    pub fn new_u8_state(vertex_id: VertexId, state: &[u8]) -> Result<Self, LabelModelError> {
        let state = U8StateVec::new(state)?;

        Ok(Label::VertexWithU8StateVec {
            vertex_id,
            state: Box::new(state),
        })
    }

    /// Gets the integer state if this label contains one.
    pub fn get_int_state(&self) -> Option<usize> {
        match self {
            Label::VertexWithIntState { state, .. } => Some(*state),
            _ => None,
        }
    }

    /// Gets the integer state vector if this label contains one.
    pub fn get_usize_state_vec(&self) -> Option<&[usize]> {
        match self {
            Label::VertexWithUsizeStateVec { state, .. } => Some(state.as_slice()),
            _ => None,
        }
    }

    /// Gets the OS-aligned state if this label contains one.
    ///
    /// # Returns
    ///
    /// Some reference to the state vector if this is a VertexWithU8StateVec, None otherwise
    pub fn get_u8_state(&self) -> Option<&[u8]> {
        match self {
            Label::VertexWithU8StateVec { state, .. } => Some(state.as_slice()),
            _ => None,
        }
    }

    pub fn vertex_id(&self) -> &VertexId {
        match self {
            Label::Vertex(vertex_id) => vertex_id,
            Label::VertexWithIntState { vertex_id, .. } => vertex_id,
            Label::VertexWithUsizeStateVec { vertex_id, .. } => vertex_id,
            Label::VertexWithU8StateVec { vertex_id, .. } => vertex_id,
        }
    }

    /// length of the label state. does not include the VertexId.
    pub fn len(&self) -> usize {
        match self {
            Label::Vertex(..) => 0,
            Label::VertexWithIntState { .. } => 1,
            Label::VertexWithUsizeStateVec { state, .. } => state.len(),
            Label::VertexWithU8StateVec { state, .. } => state.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Label::Vertex(vertex_id) => write!(f, "Vertex({vertex_id})"),
            Label::VertexWithIntState { vertex_id, state } => {
                write!(f, "VertexWithIntState({vertex_id}, {state})")
            }
            Label::VertexWithUsizeStateVec { vertex_id, state } => {
                write!(
                    f,
                    "VertexWithUsizeStateVec({vertex_id}, {:?})",
                    state.as_slice()
                )
            }
            Label::VertexWithU8StateVec { vertex_id, state } => {
                write!(
                    f,
                    "VertexWithU8StateVec({vertex_id}, {}, {:?})",
                    state.len(),
                    state.as_slice()
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use super::*;

    #[test]
    fn test_e2e_display_trip() {
        let modes = ["walk", "bike", "drive", "tnc", "transit"];
        let trip_sequence = [0, 2, 4, 0, 4, 3];
        let vertex_id = VertexId(1234);
        let label = Label::new_u8_state(vertex_id, &trip_sequence).unwrap();
        println!("label storage: {}", label);
        let out = label.get_u8_state().unwrap();
        let trip_modes = out
            .iter()
            .map(|idx| modes[*idx as usize].to_string())
            .join(",");
        println!("[{}]", trip_modes);
        assert_eq!(
            trip_modes,
            "walk,drive,transit,walk,transit,tnc".to_string()
        );
    }
}
