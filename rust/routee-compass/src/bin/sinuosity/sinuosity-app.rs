use clap::Parser;
use geo::LineString;
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

impl SinuosityAppCliArgs {
    // validates CLI args if any validation needs to be performed
    pub fn validate(&self) -> Result<(), Error> {
        todo!();
    }
}
fn main() -> Result<(), std::io::Error> {
    let args = SinuosityAppCliArgs::parse();

    let linestrings: Box<[LineString<f64>]> = read_raw_file(
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

    let edge_sinuosities: Vec<f64> = linestrings
        .clone()
        .iter()
        .map(|linestring| sinuosity(linestring))
        .collect();

    todo!(); // write the sinuosity scores to args.sinuosity_output_file

    Ok(())
}

// TODO: figure out what error to throw instead of String. that's def not right. probably some valueError?
fn sinuosity(linestring: &LineString) -> f64 {
    // linestrings.iter()
    todo!(); // implement sinuosity computation for a single linestring
}
