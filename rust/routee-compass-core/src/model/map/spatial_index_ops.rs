use super::map_error::MapError;
use crate::util::geo::haversine;
use geo::Point;
use rstar::AABB;
use uom::si::f64::Length;

pub fn test_threshold(
    envelope: &AABB<Point<f32>>,
    other: &Point<f32>,
    tolerance_distance: Length,
) -> Result<bool, MapError> {
    let this_coord = envelope.lower().0;
    let distance =
        haversine::coord_distance(&this_coord, &other.0).map_err(MapError::MapMatchError)?;
    Ok(distance <= tolerance_distance)
}

pub fn within_threshold(
    envelope: &AABB<Point<f32>>,
    other: &Point<f32>,
    tolerance_distance: Length,
) -> Result<(), MapError> {
    let this_coord = envelope.lower().0;
    let this_distance =
        haversine::coord_distance(&this_coord, &other.0).map_err(MapError::MapMatchError)?;
    if this_distance >= tolerance_distance {
        Err(MapError::MapMatchError(
            format!(
                "coord {:?} nearest vertex coord is {:?} which is {} {} away, exceeding the distance tolerance of {} {}", 
                this_coord,
                other,
                this_distance.get::<uom::si::length::meter>(),
                "meters",
                tolerance_distance.get::<uom::si::length::meter>(),
                "meters"
            )
        ))
    } else {
        Ok(())
    }
}
