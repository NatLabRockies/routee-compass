use geo::{Geometry, MapCoords, TryConvert};
use geozero::{error::GeozeroError, wkb::Wkb, ToGeo};
use serde::{Deserialize, Deserializer};
use serde_bytes;

/// Deserialize into an enum that can handle both String and Vec<u8>, in
/// that order.
#[derive(Deserialize)]
#[serde(untagged)]
enum BytesOrString {
    String(String),
    #[serde(with = "serde_bytes")]
    Bytes(Vec<u8>),
}

impl std::fmt::Display for BytesOrString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BytesOrString::Bytes(items) => write!(f, "{items:?}"),
            BytesOrString::String(s) => write!(f, "{s}"),
        }
    }
}

/// deserialize geometries from WKB strings
pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Geometry<f32>>, D::Error>
where
    D: Deserializer<'de>,
{
    let data = Option::<BytesOrString>::deserialize(deserializer)?;

    data.map(|v| {
        let bytes = match &v {
            BytesOrString::Bytes(b) => b.clone(),
            BytesOrString::String(s) => hex::decode(s).map_err(|e| {
                serde::de::Error::custom(format!("failure converting hex wkb string to bytes: {e}"))
            })?,
        };

        let g = Wkb(bytes).to_geo().map_err(|e| {
            let msg = format!("unable to parse bytes '{v}' as WKB: {e}");
            serde::de::Error::custom(msg)
        })?;

        g.try_map_coords(|geo::Coord { x, y }| {
            Ok(geo::Coord {
                x: x as f32,
                y: y as f32,
            })
        })
        .map_err(|e: GeozeroError| {
            serde::de::Error::custom(format!("Could not map coordinates for geometry {g:?}: {e}"))
        })
    })
    .transpose()
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
