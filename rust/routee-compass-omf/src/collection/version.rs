use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// OvertureMaps release identifiers
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub enum ReleaseVersion {
    #[default]
    Latest,
    Monthly {
        datetime: NaiveDate,
        version: Option<u8>,
    },
}

impl From<ReleaseVersion> for String {
    fn from(version: ReleaseVersion) -> Self {
        match version {
            ReleaseVersion::Monthly { datetime, version } => {
                format!("{}.{}", datetime.format("%Y-%m-%d"), version.unwrap_or(0)).to_string()
            }
            ReleaseVersion::Latest => "latest".into(),
        }
    }
}

impl From<&ReleaseVersion> for String {
    fn from(version: &ReleaseVersion) -> Self {
        match version {
            ReleaseVersion::Monthly { datetime, version } => {
                format!("{}.{}", datetime.format("%Y-%m-%d"), version.unwrap_or(0)).to_string()
            }
            ReleaseVersion::Latest => "latest".into(),
        }
    }
}

impl std::fmt::Display for ReleaseVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from(self))
    }
}
