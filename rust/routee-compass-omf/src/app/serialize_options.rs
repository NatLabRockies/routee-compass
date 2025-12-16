use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct OvertureSerializeOptions {
    out_file: String,
    scope: SerializeScope,
}

#[derive(Debug, Serialize, Deserialize)]
enum SerializeScope {
    Complete,
    Compass,
}
