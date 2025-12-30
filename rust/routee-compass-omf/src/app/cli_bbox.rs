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
        return Err(format!("expected format: xmin,xmax,ymin,ymax, got: {}", s));
    }

    let xmin = parts[0]
        .trim()
        .parse::<f32>()
        .map_err(|_| format!("invalid xmin: {}", parts[0]))?;
    let xmax = parts[2]
        .trim()
        .parse::<f32>()
        .map_err(|_| format!("invalid xmax: {}", parts[1]))?;
    let ymin = parts[1]
        .trim()
        .parse::<f32>()
        .map_err(|_| format!("invalid ymin: {}", parts[2]))?;
    let ymax = parts[3]
        .trim()
        .parse::<f32>()
        .map_err(|_| format!("invalid ymax: {}", parts[3]))?;

    Ok(CliBoundingBox {
        xmin,
        xmax,
        ymin,
        ymax,
    })
}

impl std::fmt::Display for CliBoundingBox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{},{},{},{}", self.xmin, self.xmax, self.ymin, self.ymax)
    }
}
