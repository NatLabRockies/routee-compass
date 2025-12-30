use geo::{Geometry, MapCoords, TryConvert};
use geozero::{error::GeozeroError, wkb::Wkb, ToGeo};
use serde::{Deserialize, Deserializer};

/// deserialize geometries from WKB strings
pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Geometry<f32>>, D::Error>
where
    D: Deserializer<'de>,
{
    // Deserialize into an enum that can handle both Vec<u8> and String
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BytesOrString {
        Bytes(Vec<u8>),
        String(String),
    }

    let data = Option::<BytesOrString>::deserialize(deserializer)?;

    data.map(|v| {
        let bytes = match v {
            BytesOrString::Bytes(b) => b,
            BytesOrString::String(s) => s.into_bytes(),
        };

        let g = Wkb(bytes).to_geo()?;

        g.try_map_coords(|geo::Coord { x, y }| {
            Ok(geo::Coord {
                x: x as f32,
                y: y as f32,
            })
        })
    })
    .transpose()
    .map_err(|e: GeozeroError| serde::de::Error::custom(format!("Could not decode wkb: {e}")))
}

pub fn serialize<S>(t: &Option<Geometry<f32>>, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    match t {
        None => s.serialize_none(),
        Some(g) => {
            let mut out_bytes = vec![];
            let geom: Geometry<f64> = g.try_convert().map_err(|e| {
                serde::ser::Error::custom(format!(
                    "unable to convert geometry from f32 to f64: {e}"
                ))
            })?;
            let write_options = wkb::writer::WriteOptions {
                endianness: wkb::Endianness::BigEndian,
            };
            wkb::writer::write_geometry(&mut out_bytes, &geom, &write_options).map_err(|e| {
                serde::ser::Error::custom(format!("failed to write geometry as WKB: {e}"))
            })?;

            let wkb_str = out_bytes
                .iter()
                .map(|b| format!("{b:02X?}"))
                .collect::<Vec<String>>()
                .join("");

            s.serialize_str(&wkb_str)
        }
    }
}
