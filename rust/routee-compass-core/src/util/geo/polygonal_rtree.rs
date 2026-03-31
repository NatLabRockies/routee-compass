use geo::{Area, BooleanOps, BoundingRect, Geometry, Intersects, Polygon, Rect};
use rstar::{primitives::Rectangle, RTree, RTreeObject, AABB};
use std::iter::Sum;

pub struct Node<C: rstar::RTreeNum + geo::CoordFloat, D> {
    pub geometry: Geometry<C>,
    pub rectangle: Rectangle<(C, C)>,
    pub data: D,
}

impl<C: rstar::RTreeNum + geo::CoordFloat, D> Node<C, D> {
    pub fn new(geometry: Geometry<C>, data: D) -> Result<Node<C, D>, String> {
        let rectangle = rect_from_geometry(&geometry)?;
        let result = Node {
            geometry,
            rectangle,
            data,
        };
        Ok(result)
    }
}

impl<C: rstar::RTreeNum + geo::CoordFloat, D> RTreeObject for Node<C, D> {
    type Envelope = AABB<(C, C)>;

    fn envelope(&self) -> Self::Envelope {
        self.rectangle.envelope()
    }
}

pub type IntersectionWithArea<'a, C, D> = Vec<(&'a Node<C, D>, C)>;

pub struct PolygonalRTree<C: rstar::RTreeNum + geo::CoordFloat, D>(RTree<Node<C, D>>);

impl<C, D> PolygonalRTree<C, D>
where
    C: rstar::RTreeNum + geo::GeoNum + geo::CoordFloat + geo::bool_ops::BoolOpsNum + Sum,
{
    pub fn new(data: Vec<(Geometry<C>, D)>) -> Result<PolygonalRTree<C, D>, String> {
        let nodes = data
            .into_iter()
            .map(|(g, d)| Node::new(g, d))
            .collect::<Result<Vec<_>, _>>()?;
        let tree = RTree::bulk_load(nodes);
        Ok(PolygonalRTree(tree))
    }

    /// tests for intersection with polygonal data in this tree. this involves two steps:
    ///   1. finding rtree rectangle envelopes that intersect the incoming geometry
    ///   2. testing intersection for each discovered geometry bounded by it's rectangle
    pub fn intersection<'a>(
        &'a self,
        g: &'a Geometry<C>,
    ) -> Result<Box<dyn Iterator<Item = &'a Node<C, D>> + 'a>, String> {
        let query = rect_from_geometry(g)?;
        let iter = self
            .0
            .locate_in_envelope_intersecting(&query.envelope())
            .filter(|node| node.geometry.intersects(g));
        Ok(Box::new(iter))
    }

    pub fn intersection_with_overlap_area<'a>(
        &'a self,
        query: &'a Geometry<C>,
    ) -> Result<IntersectionWithArea<'a, C, D>, String> {
        // get all polygons in the query geometry
        let query_polygons: Vec<Polygon<C>> = match query {
            Geometry::Polygon(p) => Ok(vec![p.clone()]),
            Geometry::MultiPolygon(mp) => Ok(mp.0.clone()),
            // Geometry::GeometryCollection(geometry_collection) => todo!(),
            _ => Err(String::from(
                "areal proportion query must be performed on polygonal data",
            )),
        }?;

        // compute the overlap area for each query polygon for each geometry
        // found to intersect the query in the rtree
        let result = self
            .intersection(query)?
            .map(|node| {
                let overlap_areas = query_polygons
                    .iter()
                    .map(|p| {
                        let area = overlap_area(p, &node.geometry)?;
                        Ok(area)
                    })
                    .collect::<Result<Vec<C>, String>>()?;
                let overlap_area: C = overlap_areas.into_iter().sum();
                Ok((node, overlap_area))
            })
            .collect::<Result<Vec<(&Node<C, D>, C)>, String>>()?;

        Ok(result)
    }
}

/// helper function to create a rectangular rtree envelope for a given geometry
fn rect_from_geometry<C: rstar::RTreeNum + geo::CoordFloat>(
    g: &Geometry<C>,
) -> Result<Rectangle<(C, C)>, String> {
    let bbox_vec: Rect<C> = g
        .bounding_rect()
        .ok_or_else(|| String::from("internal error: cannot get bounds of geometry"))?;

    let envelope = Rectangle::from_corners(bbox_vec.min().x_y(), bbox_vec.max().x_y());
    Ok(envelope)
}

/// helper for computing the overlap area, which is the area that two geometries have in common
/// (the intersection).
fn overlap_area<C>(query: &Polygon<C>, overlapping: &Geometry<C>) -> Result<C, String>
where
    C: rstar::RTreeNum + geo::CoordFloat + geo::bool_ops::BoolOpsNum + Sum,
{
    match overlapping {
        Geometry::Polygon(overlap_p) => {
            let overlap = query.intersection(overlap_p);
            let overlap_area: C = overlap.iter().map(|p| p.unsigned_area()).sum();
            Ok(overlap_area)
        }
        Geometry::MultiPolygon(mp) => {
            let overlaps = mp
                .iter()
                .map(|overlapping_p| {
                    overlap_area(query, &geo::Geometry::Polygon(overlapping_p.clone()))
                })
                .collect::<Result<Vec<_>, String>>()?;
            let overlap_area: C = overlaps.into_iter().sum();
            Ok(overlap_area)
        }
        _ => Err(String::from("polygonal rtree node must be polygonal!")),
    }
}
