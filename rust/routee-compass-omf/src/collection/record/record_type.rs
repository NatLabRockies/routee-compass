pub enum OvertureRecordType {
    Places,
    Buildings,
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
        }
    }
}
