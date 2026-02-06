use std::fmt::Display;

use allocative::Allocative;
use serde::Serialize;

use crate::model::{label::label_model_error::LabelModelError, network::VertexId};

/// The required length for OS-aligned state vectors.
/// This is the word size of the target architecture.
#[cfg(target_pointer_width = "32")]
pub const OS_ALIGNED_STATE_LEN: usize = 4;

#[cfg(target_pointer_width = "64")]
pub const OS_ALIGNED_STATE_LEN: usize = 8;

#[derive(PartialEq, Eq, Hash, Debug, Clone, Serialize, Allocative)]
pub struct IntStateVec(pub Vec<usize>);

#[derive(PartialEq, Eq, Hash, Debug, Clone, Serialize, Allocative)]
pub struct U8StateVec {
    pub state: Vec<u8>,
    pub state_len: u8,
}

#[derive(PartialEq, Eq, Hash, Debug, Clone, Serialize, Allocative)]
pub enum Label {
    Vertex(VertexId),
    VertexWithIntState {
        vertex_id: VertexId,
        state: usize,
    },
    VertexWithIntStateVec {
        vertex_id: VertexId,
        state: Box<IntStateVec>,
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
    /// Creates a new VertexWithU8StateVec with validation.
    ///
    /// # Arguments
    ///
    /// * `vertex_id` - The vertex identifier
    /// * `state` - The u8 state vector, must be exactly OS_ALIGNED_STATE_LEN bytes long
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
        let mut label_state = state.to_vec();
        let state_len: u8 = state
            .len()
            .try_into()
            .map_err(|_| LabelModelError::BadLabelVecSize(state.len(), u8::MAX as usize))?;

        // Calculate total memory needed: state data + 1 byte for state_len
        let total_data_len = label_state.len() + 1;
        let remainder = total_data_len % OS_ALIGNED_STATE_LEN;
        if remainder != 0 {
            let padding_needed = OS_ALIGNED_STATE_LEN - remainder;
            label_state.extend(vec![0u8; padding_needed]);
        }

        Ok(Label::VertexWithU8StateVec {
            vertex_id,
            state: Box::new(U8StateVec {
                state: label_state,
                state_len,
            }),
        })
    }

    /// Gets the OS-aligned state if this label contains one.
    ///
    /// # Returns
    ///
    /// Some reference to the state vector if this is a VertexWithU8StateVec, None otherwise
    pub fn get_u8_state(&self) -> Option<&[u8]> {
        match self {
            Label::VertexWithU8StateVec { state, .. } => {
                let len: usize = state.state_len.into();
                Some(&state.state[0..len])
            }
            _ => None,
        }
    }

    pub fn vertex_id(&self) -> &VertexId {
        match self {
            Label::Vertex(vertex_id) => vertex_id,
            Label::VertexWithIntState { vertex_id, .. } => vertex_id,
            Label::VertexWithIntStateVec { vertex_id, .. } => vertex_id,
            Label::VertexWithU8StateVec { vertex_id, .. } => vertex_id,
        }
    }

    /// Returns true if this label variant should be stored in the vertex->labels mapping
    /// of the SearchTree.
    ///
    /// Label::Vertex is excluded because it's redundant - the vertex ID is already
    /// the key, so storing Label::Vertex(id) under key `id` provides no additional value.
    pub fn needs_vertex_map_storage(&self) -> bool {
        !matches!(self, Label::Vertex(_))
    }

    /// returns true if this label variant is not a bijection to the vertex set.
    /// if not, then its type has a greater cardinality than the vertex set and so
    /// we will want to prune any dominated labels with matching VertexId.
    pub fn does_not_require_pruning(&self) -> bool {
        !matches!(self, Label::Vertex(_))
    }
}

impl Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Label::Vertex(vertex_id) => write!(f, "Vertex({vertex_id})"),
            Label::VertexWithIntState { vertex_id, state } => {
                write!(f, "VertexWithIntState({vertex_id}, {state})")
            }
            Label::VertexWithIntStateVec { vertex_id, state } => {
                write!(f, "VertexWithIntStateVec({vertex_id}, {:?})", state.0)
            }
            Label::VertexWithU8StateVec { vertex_id, state } => {
                write!(
                    f,
                    "VertexWithU8StateVec({vertex_id}, {}, {:?})",
                    state.state_len, state.state
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

    #[test]
    fn test_new_u8_state_valid() {
        let vertex_id = VertexId(42);
        let state = vec![0u8; OS_ALIGNED_STATE_LEN];

        let label = Label::new_u8_state(vertex_id, &state).expect("test failed");

        assert_eq!(label.vertex_id(), &vertex_id);
        assert_eq!(label.get_u8_state(), Some(state.as_slice()));
    }

    #[test]
    fn test_new_u8_state_aligned() {
        let vertex_id = VertexId(42);
        let state = vec![1, 2, 3];

        let label = Label::new_u8_state(vertex_id, &state).expect("test failed");
        match label {
            Label::VertexWithU8StateVec {
                state: inner_state, ..
            } => {
                // should pad to OS_ALIGNED_STATE_LEN - 1
                // (minus one due to storing state_len value)
                assert_eq!(inner_state.state.len(), super::OS_ALIGNED_STATE_LEN - 1);
            }
            _ => panic!("wrong label variant!"),
        };
    }

    #[test]
    fn test_u8_state_none_for_other_variants() {
        let vertex_id = VertexId(42);

        let vertex_label = Label::Vertex(vertex_id);
        assert_eq!(vertex_label.get_u8_state(), None);

        let int_state_label = Label::VertexWithIntState {
            vertex_id,
            state: 123,
        };
        assert_eq!(int_state_label.get_u8_state(), None);

        // Test that VertexWithU8StateVec does return the state
        let valid_state = vec![1, 2, 3];
        let u8_vec_label = Label::new_u8_state(vertex_id, &valid_state).expect("test failed");
        let result = u8_vec_label.get_u8_state().unwrap();
        let expected = [1, 2, 3];
        assert_eq!(result, &expected);
    }

    #[test]
    fn test_u8_state_label_display() {
        let vertex_id = VertexId(42);
        let state = vec![1u8; OS_ALIGNED_STATE_LEN];
        let label = Label::new_u8_state(vertex_id, &state).expect("test failed");

        let display_string = format!("{}", label);
        assert!(display_string.contains("VertexWithU8StateVec"));
        assert!(display_string.contains("42"));
    }
}
