use opening_hours_syntax::rules::OpeningHoursExpression;
use serde::{Deserialize, Serialize};

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
