use clap::Parser;
use geo::algorithm::line_measures::{Haversine, Length};
use geo::{Distance, LineString};
use routee_compass_core::util::fs::read_utils::read_raw_file;
use std::io::{Error, ErrorKind};
use wkt::TryFromWkt;

/// From a set of WKT LineStrings in a .txt or .gz file,
/// this CLI app will compute the sinuosity of each of
/// the linestrings.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct SinuosityAppCliArgs {
    /// Enumerate edge geometries .gz file
    #[arg(long, default_value = "edges-geometries-enumerated.txt.gz")]
    pub geometry_input_file: String,
    /// Output file containing sinuosity information
    #[arg(long, default_value = "edges-sinuosity-enumerated.txt")]
    pub sinuosity_output_file: String,
}

fn sinuosity(linestring: &LineString<f32>) -> Result<f32, String> {
    let origin = linestring
        .points()
        .next()
        .ok_or("Something happened with the first point in the linestring.".to_string())?;

    let destination = linestring
        .points()
        .next_back()
        .ok_or("Something happened with the last point in the linestring.".to_string())?;

    let distance_origin_to_destination = Haversine.distance(origin, destination);
    let length_linestring = Haversine.length(linestring);

    if distance_origin_to_destination == 0.0 {
        Ok(f32::INFINITY) // the path is a loop.
    } else {
        let sinuosity = length_linestring / distance_origin_to_destination;
        Ok(sinuosity.max(1.0f32)) // sinuosity cannot go below 1
    }
}

/// sinuosity-app is a simple CLI app that allows the user to pass in a
/// file containing edge geometries as WKT linestrings and compute the
/// sinuosity of each edge in the set.
fn main() -> Result<(), std::io::Error> {
    let args = SinuosityAppCliArgs::parse();

    // read in the raw file (GZIP'd or regular) containing WKT linestrings on each row
    let linestrings: Box<[LineString<f32>]> = read_raw_file(
        args.geometry_input_file.clone(),
        |_, line| {
            LineString::try_from_wkt_str(&line).map_err(|e| {
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("{e} - data not in WKT LineString format"),
                )
            })
        },
        None,
        None,
    )?;

    // Computes the sinuosity of each edge and concatenates the results into a
    // single string, where each row holds the sinuosity of the corresponding edge.
    let sinuosities = linestrings
        .iter()
        // double mapcd s: linestring->sinuosity->string (for file writing)
        .map(|ls| sinuosity(ls).map(|n| n.to_string()))
        .collect::<Result<Vec<String>, String>>()
        .map_err(|e| {
            Error::new(
                ErrorKind::InvalidData,
                format!("error in computing sinuosity {e}"),
            )
        })?
        // join with newlines to print an single sinuosity value for each row
        .join("\n");

    std::fs::write(&args.sinuosity_output_file, sinuosities)?;

    Ok(())
}

#[cfg(test)]
// Geographic coordinates are written at full precision for readability even
// though the app operates on f32; the extra digits are intentional.
#[allow(clippy::excessive_precision)]
mod tests {
    use super::*;
    use geo::line_string;
    use geo::{Destination, Haversine, Point};

    #[test]
    fn test_infty_on_loop() -> Result<(), String> {
        // Loop centered in Golden, CO.
        let linestring = line_string![
            (x: -105.2208663, y: 39.7555000),
            (x: -105.2208976, y: 39.7555898),
            (x: -105.2209832, y: 39.7556556),
            (x: -105.2211000, y: 39.7556797),
            (x: -105.2212168, y: 39.7556556),
            (x: -105.2213024, y: 39.7555898),
            (x: -105.2213337, y: 39.7555000),
            (x: -105.2213024, y: 39.7554102),
            (x: -105.2212168, y: 39.7553444),
            (x: -105.2211000, y: 39.7553203),
            (x: -105.2209832, y: 39.7553444),
            (x: -105.2208976, y: 39.7554102),
            (x: -105.2208663, y: 39.7555000),
        ];

        assert_eq!(f32::INFINITY, sinuosity(&linestring)?);
        Ok(())
    }

    #[test]
    fn test_1_on_straight() -> Result<(), String> {
        let epsilon: f32 = 0.001;
        let upper: f32 = 1.0 + epsilon;
        let lower: f32 = 1.0 - epsilon;

        // Straight line down Washington Ave. in Golden, CO.
        let linestring = line_string![
            (x: 39.749683, y: -105.216019),
            (x: 39.756780, y: -105.222675),
        ];

        assert!(sinuosity(&linestring)? <= upper && sinuosity(&linestring)? >= lower);
        Ok(())
    }

    #[test]
    fn test_sqrt2_on_isosceles() -> Result<(), String> {
        // given isosceles triangle with base length 1km, the sinuosity of the
        // linestring containing the adjacent sides should be ~sqrt(2) m if
        // the distance from the midpoint of the base to the intersection
        // of the adjacent sides is 500m.

        // note! f32 + haversine implictly constrains us to a certain level of precision.
        // this is why we opted for a base of 1000m instead of 1m.

        let epsilon: f32 = 0.001;
        let upper = 2.0f32.sqrt() + epsilon;
        let lower = 2.0f32.sqrt() - epsilon;

        // base (1 km)
        let origin = Point::new(-105.222682, 39.756827); // centered in golden
        let destination = Haversine.destination(origin, 0.0, 1000.0); // distance in meters, due north

        // midpoint of base
        let midpoint = Haversine.destination(origin, 0.0, 500.0);

        // extend a point perpendicular to the midpoint (due east, 500m above base).
        let apex = Haversine.destination(midpoint, 90.0, 500.0);

        // connect the dots LineString (o-a, a-o)
        let linestring = LineString::from(vec![origin, apex, destination]);
        let sinuosity = sinuosity(&linestring)?;
        assert!(sinuosity <= upper && sinuosity >= lower);

        Ok(())
    }
}
