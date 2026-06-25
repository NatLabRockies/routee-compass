use clap::Parser;
use geo::algorithm::line_measures::{Haversine, Length};
use geo::{Distance, LineString};
use routee_compass_core::util::fs::read_utils::read_raw_file;
use std::io::{Error, ErrorKind};
use wkt::TryFromWkt;

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
    let first_coord = linestring
        .points()
        .next()
        .ok_or("Something happened with the first point in the linestring.".to_string())?;

    let last_coord = linestring
        .points()
        .last()
        .ok_or("Something happened with the last point in the linestring.".to_string())?;

    let haversine_distance = Haversine.distance(first_coord, last_coord);
    let haversine_length = Haversine.length(linestring);

    if haversine_distance == 0.0 {
        Ok(f32::INFINITY)
    } else {
        Ok(haversine_distance / haversine_length)
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

    // computes the sinuosity of each linestring
    let sinuosities: Vec<f32> = linestrings
        .clone()
        .iter()
        .map(sinuosity)
        .collect::<Result<_, _>>()
        .map_err(|e| {
            Error::new(
                ErrorKind::InvalidData,
                format!("error in computing sinuosity {e}"),
            )
        })?;

    for val in sinuosities {
        println!("{val}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::line_string;

    #[test]
    fn test_infty_on_loop() -> Result<(), String> {
        // Loop centered in Golden, CO.
        let test_loop = line_string![
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

        assert_eq!(f32::INFINITY, sinuosity(&test_loop)?);
        Ok(())
    }

    #[test]
    fn test_1_on_straight() -> Result<(), String> {
        let epsilon: f32 = 0.001;
        let upper: f32 = 1.0 + epsilon;
        let lower: f32 = 1.0 - epsilon;

        // Straight line down Washington Ave. in Golden, CO.
        let test_line = line_string![
            (x: 39.749683, y: -105.216019),
            (x: 39.756780, y: -105.222675),
        ];

        let sinuosity = sinuosity(&test_line)?;
        assert!(sinuosity <= upper && sinuosity >= lower);
        Ok(())
    }

    #[test]
    fn test_sqrt2_on_isosceles() -> Result<(), String> {
        let epsilon: f32 = 0.001;
        let upper: f32 = 2.0_f32.sqrt() + epsilon;
        let lower: f32 = 2.0_f32.sqrt() - epsilon;

        let triangle = line_string![
            (x: 0.0, y: 0.0),   // src
            (x: 0.5, y: 0.5),   // apex (right angle)
            (x: 1.0, y: 0.0),   // dst
        ];

        let sinuosity = sinuosity(&triangle)?;
        assert!(sinuosity <= upper && sinuosity >= lower);
        Ok(())
    }
}
