use routee_compass_core::model::network::Vertex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct VertexSerializable {
    vertex_id: usize,
    x: f32,
    y: f32,
}

impl From<Vertex> for VertexSerializable {
    fn from(value: Vertex) -> Self {
        Self {
            vertex_id: value.vertex_id.0,
            x: value.x(),
            y: value.y(),
        }
    }
}
