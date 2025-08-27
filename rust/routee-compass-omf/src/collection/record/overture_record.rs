use super::{BuildingsRecord, PlacesRecord};

#[derive(Debug)]
pub enum OvertureRecord {
    Places(PlacesRecord),
    Buildings(BuildingsRecord),
}
