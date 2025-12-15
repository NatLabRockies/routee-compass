use uom::si::f64::Length;


pub struct Match {
    pub trace_index: usize,
    pub map_id: usize,
    pub match_distance: Length,
}

pub struct MapMatchingResult {
    pub matches: Vec<Match>
}