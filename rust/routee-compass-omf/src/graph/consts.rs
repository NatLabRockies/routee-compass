/// A distance tolerance (in meters) used to mitigate floating-point drift when extracting
/// LineString geometries for segment splits.
///
/// When computing the coordinates that strictly lie between `src` and `dst` connectors,
/// the cumulative sum of individual line segment lengths (`total_distance += line_distance`)
/// can slightly exceed the directly computed `distance_to_src` due to floating-point
/// inaccuracies.
///
/// If this tolerance is not applied, the algorithm may falsely identify the starting
/// coordinate of the split as an intermediate point. This results in pushing the exact
/// same coordinate twice in a row, creating a 0-distance segment fragment that can
/// mangle downstream network computations such as intersection bearings.
pub const F32_DISTANCE_TOLERANCE: f32 = 1e-6;

/// same intent as [F32_DISTANCE_TOLERANCE] for f64 values.
pub const F64_DISTANCE_TOLERANCE: f64 = 1e-6;
