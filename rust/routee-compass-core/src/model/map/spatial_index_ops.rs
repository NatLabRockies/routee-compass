use super::map_error::MapError;
use crate::util::geo::haversine;
use geo::{Coord, Point};
use rstar::AABB;
use uom::si::f64::Length;

pub fn test_threshold(
    envelope: &AABB<Point<f32>>,
    other: &Point<f32>,
    tolerance_distance: Length,
) -> Result<bool, MapError> {
    let min = envelope.lower().0;
    let max = envelope.upper().0;
    let p = other.0;

    let clamped_x = p.x.max(min.x).min(max.x);
    let clamped_y = p.y.max(min.y).min(max.y);
    let closest_point_on_aabb = Coord {
        x: clamped_x,
        y: clamped_y,
    };

    let distance =
        haversine::coord_distance(&closest_point_on_aabb, &p).map_err(MapError::MapMatchError)?;
    Ok(distance <= tolerance_distance)
}

pub fn within_threshold(
    envelope: &AABB<Point<f32>>,
    other: &Point<f32>,
    tolerance_distance: Length,
) -> Result<(), MapError> {
    let min = envelope.lower().0;
    let max = envelope.upper().0;
    let p = other.0;

    let clamped_x = p.x.max(min.x).min(max.x);
    let clamped_y = p.y.max(min.y).min(max.y);
    let closest_point_on_aabb = Coord {
        x: clamped_x,
        y: clamped_y,
    };

    let this_distance =
        haversine::coord_distance(&closest_point_on_aabb, &p).map_err(MapError::MapMatchError)?;
    if this_distance > tolerance_distance {
        Err(MapError::MapMatchError(
            format!(
                "coord {:?} nearest point on AABB is {:?} which is {} {} away, exceeding the distance tolerance of {} {}", 
                other,
                closest_point_on_aabb,
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

#[cfg(test)]
mod test {
    use super::*;
    use geo::{coord, Point};
    use rstar::AABB;
    use uom::si::f64::Length;
    use uom::si::length::meter;

    #[test]
    fn test_threshold_upper_corner() {
        let envelope = AABB::from_corners(
            Point(coord! { x: 0.0, y: 0.0 }),
            Point(coord! { x: 1.0, y: 1.0 }),
        );
        let query_point = Point(coord! { x: 1.0, y: 1.0 });
        let tolerance = Length::new::<meter>(1.0);

        let result = test_threshold(&envelope, &query_point, tolerance).unwrap();
        assert!(result, "Point at upper corner should be within threshold");
    }

    #[test]
    fn test_within_threshold_upper_corner() {
        let envelope = AABB::from_corners(
            Point(coord! { x: 0.0, y: 0.0 }),
            Point(coord! { x: 1.0, y: 1.0 }),
        );
        let query_point = Point(coord! { x: 1.0, y: 1.0 });
        let tolerance = Length::new::<meter>(1.0);

        let result = within_threshold(&envelope, &query_point, tolerance);
        assert!(
            result.is_ok(),
            "Point at upper corner should be within threshold, but got error: {:?}",
            result.err()
        );
    }
}
