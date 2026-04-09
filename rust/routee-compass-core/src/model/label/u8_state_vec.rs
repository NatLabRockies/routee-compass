use allocative::Allocative;
use serde::Serialize;

use crate::model::label::LabelModelError;

/// The required length for OS-aligned state vectors.
/// This is the word size of the target architecture.
#[cfg(target_pointer_width = "32")]
pub const OS_ALIGNED_STATE_LEN: usize = 4;

#[cfg(target_pointer_width = "64")]
pub const OS_ALIGNED_STATE_LEN: usize = 8;

#[derive(Debug, Clone, Serialize, Allocative)]
pub struct U8StateVec {
    /// the memory-padded storage
    state: Vec<u8>,
    /// the length of usable storage dedicated to this state, <= self.state.len()
    state_len: u8,
}

impl PartialEq for U8StateVec {
    fn eq(&self, other: &Self) -> bool {
        let len = self.state_len as usize;
        let other_len = other.state_len as usize;
        len == other_len && self.state[..len] == other.state[..other_len]
    }
}

impl Eq for U8StateVec {}

impl std::hash::Hash for U8StateVec {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let len = self.state_len as usize;
        self.state_len.hash(state);
        self.state[..len].hash(state);
    }
}

impl U8StateVec {
    /// creates a U8StateVec directly from these values. additional slots may be padded into
    /// the underlying representation in order to meet memory alignment guarantees.
    pub fn new(state: &[u8]) -> Result<Self, LabelModelError> {
        let state_len: u8 = state
            .len()
            .try_into()
            .map_err(|_| LabelModelError::BadLabelVecSize(state.len(), u8::MAX as usize))?;

        // Calculate total memory needed: state data + 1 byte for state_len metadata
        let total_data_len = state.len() + 1;
        let remainder = total_data_len % OS_ALIGNED_STATE_LEN;
        let padding_needed = if remainder != 0 {
            OS_ALIGNED_STATE_LEN - remainder
        } else {
            0
        };

        let mut v = Vec::with_capacity(state.len() + padding_needed);
        v.extend_from_slice(state);
        v.resize(state.len() + padding_needed, 0);

        Ok(U8StateVec {
            state: v,
            state_len,
        })
    }

    /// gets a value from this vector. memory padding slots are ignored.
    pub fn get(&self, index: usize) -> Option<u8> {
        if index < self.state_len as usize {
            self.state.get(index).cloned()
        } else {
            None
        }
    }

    /// length of this state vector (not including memory padding slots).
    pub fn len(&self) -> usize {
        self.state_len as usize
    }

    /// if the state representation used here is actually empty.
    pub fn is_empty(&self) -> bool {
        self.state_len == 0
    }

    /// retrieve the state as a reference to a slice. only returns values up to self.state_len.
    pub fn as_slice(&self) -> &[u8] {
        let len = self.state_len as usize;
        &self.state[..len]
    }

    /// shows the length of the underlying vector, including unused slots that exist only
    /// for OS memory alignment. only intended for debugging purposes.
    pub fn storage_len(&self) -> usize {
        self.state.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_vec() {
        let u8_state_vec = U8StateVec::new(&[]).expect("empty vec should be valid");
        assert!(u8_state_vec.is_empty());
        assert_eq!(u8_state_vec.len(), 0);
        assert_eq!(u8_state_vec.get(0), None);
        // Should still have padding to OS_ALIGNED_STATE_LEN
        assert!(u8_state_vec.storage_len() >= 1);
    }

    #[test]
    fn test_max_size_limit() {
        // 255 is valid for u8
        let max_valid = vec![0u8; 255];
        assert!(U8StateVec::new(&max_valid).is_ok());

        // 256 exceeds u8 capacity
        let too_big = vec![0u8; 256];
        let result = U8StateVec::new(&too_big);
        assert!(matches!(
            result,
            Err(LabelModelError::BadLabelVecSize(256, 255))
        ));
    }

    #[test]
    fn test_padding_exact_fit() {
        // On 64-bit, metadata (1) + data (7) = 8. Zero padding needed.
        // On 32-bit, metadata (1) + data (3) = 4. Zero padding needed.
        let data_len = OS_ALIGNED_STATE_LEN - 1;
        let data = vec![0u8; data_len];
        let vec = U8StateVec::new(&data).unwrap();
        assert_eq!(vec.storage_len(), data_len);
    }

    #[test]
    fn test_new_u8_state_valid() {
        let state = vec![0u8; OS_ALIGNED_STATE_LEN];
        let u8_state_vec = U8StateVec::new(&state).expect("test failed");
        assert_eq!(u8_state_vec.as_slice(), state.as_slice());
    }

    #[test]
    fn test_new_u8_state_aligned() {
        let state = vec![1, 2, 3];
        let u8_state_vec = U8StateVec::new(&state).expect("test failed");

        // should pad to OS_ALIGNED_STATE_LEN - 1
        // (minus one due to storing state_len value)
        assert_eq!(u8_state_vec.storage_len(), OS_ALIGNED_STATE_LEN - 1);
        assert_eq!(u8_state_vec.len(), 3);
        assert_eq!(u8_state_vec.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn test_get() {
        let state = vec![10, 20, 30];
        let u8_state_vec = U8StateVec::new(&state).expect("test failed");

        assert_eq!(u8_state_vec.get(0), Some(10));
        assert_eq!(u8_state_vec.get(1), Some(20));
        assert_eq!(u8_state_vec.get(2), Some(30));
        assert_eq!(u8_state_vec.get(3), None);
    }

    #[test]
    fn test_eq_and_hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let state1 = vec![1, 2, 3];
        let state2 = vec![1, 2, 3];
        let state3 = vec![1, 2, 4];

        let vec1 = U8StateVec::new(&state1).unwrap();
        let vec2 = U8StateVec::new(&state2).unwrap();
        let vec3 = U8StateVec::new(&state3).unwrap();

        assert_eq!(vec1, vec2);
        assert_ne!(vec1, vec3);

        let mut h1 = DefaultHasher::new();
        vec1.hash(&mut h1);
        let mut h2 = DefaultHasher::new();
        vec2.hash(&mut h2);
        assert_eq!(h1.finish(), h2.finish());
    }
}
