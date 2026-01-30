use opening_hours_syntax::rules::OpeningHoursExpression;
use serde::{Deserialize, Serialize};

/// the opening hours for an OvertureMaps record. because of the existence of
/// invalid data, this is provided with a fallback Unexpected variant when
/// parsing as opening hours fails. see documentation for the OSM hours for
/// more information.
///
/// <https://wiki.openstreetmap.org/wiki/Key:opening_hours>
#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(untagged)]
pub enum DuringExpression {
    #[serde(with = "opening_hours_codec")]
    Osm(OpeningHoursExpression),
    Unexpected(String),
}

mod opening_hours_codec {
    use opening_hours_syntax::rules::OpeningHoursExpression;
    use serde::Deserialize;
    pub fn serialize<S>(t: &OpeningHoursExpression, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        s.serialize_str(&t.to_string())
    }
    pub fn deserialize<'de, D>(d: D) -> Result<OpeningHoursExpression, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(d)?;
        opening_hours_syntax::parse(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_opening_hours() {
        let json = r#""07:00-16:00""#;
        let result: DuringExpression = serde_json::from_str(json).unwrap();
        assert!(matches!(result, DuringExpression::Osm(_)));
    }

    #[test]
    fn test_parse_invalid_opening_hours_as_unexpected() {
        let json = r#""sunset""#;
        let result: DuringExpression = serde_json::from_str(json).unwrap();
        assert!(matches!(result, DuringExpression::Unexpected(s) if s == "sunset"));
    }
}
