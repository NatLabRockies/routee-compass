use routee_compass::app::compass::CompassAppConfig;

/// writes the schema of the Compass CLI to a
pub fn main() {
    let schema = schemars::schema_for!(CompassAppConfig);
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
