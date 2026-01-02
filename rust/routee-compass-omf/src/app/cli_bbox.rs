use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CliBoundingBox {
    pub xmin: f32,
    pub xmax: f32,
    pub ymin: f32,
    pub ymax: f32,
}

pub fn parse_bbox(s: &str) -> Result<CliBoundingBox, String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return Err(format!("expected format: xmin,xmax,ymin,ymax, got: {s}"));
    }

    let xmin = parse_lon(parts[0])?;
    let xmax = parse_lon(parts[1])?;

    let ymin = parse_lat(parts[2])?;
    let ymax = parse_lat(parts[3])?;

    if !(xmin < xmax) {
        Err(format!(
            "bbox: xmin must be less than xmax, but found [{xmin},{xmax}]"
        ))
    } else if !(ymin < ymax) {
        Err(format!(
            "bbox: ymin must be less than ymax, but found [{ymin},{ymax}]"
        ))
    } else {
        Ok(CliBoundingBox {
            xmin,
            xmax,
            ymin,
            ymax,
        })
    }
}

impl std::fmt::Display for CliBoundingBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{},{},{}", self.xmin, self.xmax, self.ymin, self.ymax)
    }
}

fn parse_lat(lat: &str) -> Result<f32, String> {
    parse_num(lat, -90.0, 90.0).map_err(|e| format!("invalid latitude: {e}"))
}

fn parse_lon(lat: &str) -> Result<f32, String> {
    parse_num(lat, -180.0, 180.0).map_err(|e| format!("invalid longitude: {e}"))
}

fn parse_num(s: &str, min: f32, max: f32) -> Result<f32, String> {
    let v = s
        .trim()
        .parse::<f32>()
        .map_err(|_| format!("not a number: {s}"))?;
    if v < min || max < v {
        Err(format!(
            "number '{v}' is not valid, must be in range [{min},{max}]"
        ))
    } else {
        Ok(v)
    }
}
