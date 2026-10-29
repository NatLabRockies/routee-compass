use super::{map_error::MapError, spatial_index_ops as ops};
use crate::model::network::{Edge, EdgeId, EdgeListId};
use geo::{BoundingRect, LineString, Point};
use rstar::{PointDistance, RTreeObject, AABB};
use uom::si::f64::Length;

/// rtree element for edge-oriented map matching.
#[derive(Clone)]
pub struct MapEdgeRTreeObject {
    pub edge_list_id: EdgeListId,
    pub edge_id: EdgeId,
    pub envelope: AABB<(f32, f32)>,
}

impl MapEdgeRTreeObject {
    pub fn new(edge: &Edge, linestring: &LineString<f32>) -> MapEdgeRTreeObject {
        let envelope = match linestring.bounding_rect() {
            Some(rect) => AABB::from_corners(rect.min().x_y(), rect.max().x_y()),
            None => AABB::from_corners((f32::MIN, f32::MIN), (f32::MAX, f32::MAX)),
        };
        MapEdgeRTreeObject {
            edge_list_id: edge.edge_list_id,
            edge_id: edge.edge_id,
            envelope,
        }
    }

    pub fn test_threshold(
        &self,
        point: &Point<f32>,
        tolerance: &Option<Length>,
    ) -> Result<bool, MapError> {
        match tolerance {
            Some(dist) => ops::test_threshold(&self.envelope, point, *dist),
            None => Ok(true),
        }
    }

    pub fn within_distance_threshold(
        &self,
        point: &Point<f32>,
        tolerance: &Option<Length>,
    ) -> Result<(), MapError> {
        match tolerance {
            Some(dist) => ops::within_threshold(&self.envelope, point, *dist),
            None => Ok(()),
        }
    }
}

impl RTreeObject for MapEdgeRTreeObject {
    type Envelope = AABB<(f32, f32)>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

impl PointDistance for MapEdgeRTreeObject {
    fn distance_2(&self, point: &(f32, f32)) -> f32 {
        self.envelope.distance_2(point)
    }
}
