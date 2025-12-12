pub enum OvertureRecordType {
    Places,
    Buildings,
    Segment,
    Connector,
}

impl OvertureRecordType {
    pub fn format_url(&self, release_str: String) -> String {
        match self {
            OvertureRecordType::Places => {
                format!("release/{release_str}/theme=places/type=place/").to_owned()
            }
            OvertureRecordType::Buildings => {
                format!("release/{release_str}/theme=buildings/type=building/").to_owned()
            }
            OvertureRecordType::Segment => {
                format!("release/{release_str}/theme=transportation/type=segment/").to_owned()
            }
            OvertureRecordType::Connector => {
                format!("release/{release_str}/theme=transportation/type=connector/").to_owned()
            }
        }
    }
}
