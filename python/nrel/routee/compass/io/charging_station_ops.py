import importlib.resources
import logging
import re
from typing import TYPE_CHECKING, Union

import requests

if TYPE_CHECKING:
    from geopandas import GeoDataFrame
    from pandas import DataFrame
    from shapely.geometry import Polygon

log = logging.getLogger(__name__)


def download_ev_charging_stations(state: str, api_key: str = "DEMO_KEY") -> "DataFrame":
    """
    Download EV charging stations for a given state and return processed dataframe.

    Parameters:
    -----------
    state : str
        Two-letter state code (e.g., 'CO', 'CA', 'TX')
    api_key : str, optional
        NREL API key. Defaults to 'DEMO_KEY' which has rate limits.

    Returns:
    --------
    pd.DataFrame
        DataFrame with columns: latitude, longitude, power_type, power_kw, cost_per_kwh
        Each row represents one charging station/power type combination.
    """
    try:
        import pandas as pd
    except ImportError as _:
        raise ImportError("Required libraries not installed. Please install pandas.")
    # Build query URL
    query = f"https://developer.nlr.gov/api/alt-fuel-stations/v1.json?api_key={api_key}&status=E&access=public&fuel_type=ELEC&state={state}"

    # Download data
    try:
        response = requests.get(query)
        response.raise_for_status()
        result = response.json()
    except requests.RequestException as e:
        raise RuntimeError(f"Failed to download charging station data: {e}")

    if "fuel_stations" not in result:
        raise RuntimeError("Invalid response format from NREL API")

    # Create dataframe
    df = pd.DataFrame(result["fuel_stations"])

    if df.empty:
        return pd.DataFrame(
            columns=["latitude", "longitude", "power_type", "power_kw", "cost_per_kwh"]
        )

    # Select relevant columns
    required_cols = [
        "latitude",
        "longitude",
        "ev_level1_evse_num",
        "ev_level2_evse_num",
        "ev_dc_fast_num",
        "ev_pricing",
    ]
    df = df[required_cols]

    # Process each station and create rows for each power type available
    processed_rows = []

    for _index, station in df.iterrows():
        lat, lon = station["latitude"], station["longitude"]
        pricing = station["ev_pricing"]
        cost_per_kwh = _parse_cost_per_kwh(pricing)

        # Add L1 stations
        if (
            pd.notna(station["ev_level1_evse_num"])
            and station["ev_level1_evse_num"] > 0
        ):
            processed_rows.append(
                {
                    "latitude": lat,
                    "longitude": lon,
                    "power_type": "L1",
                    "power_kw": 1.8,
                    "cost_per_kwh": cost_per_kwh,
                }
            )

        # Add L2 stations
        if (
            pd.notna(station["ev_level2_evse_num"])
            and station["ev_level2_evse_num"] > 0
        ):
            processed_rows.append(
                {
                    "latitude": lat,
                    "longitude": lon,
                    "power_type": "L2",
                    "power_kw": 7.2,
                    "cost_per_kwh": cost_per_kwh,
                }
            )

        # Add DCFC stations
        if pd.notna(station["ev_dc_fast_num"]) and station["ev_dc_fast_num"] > 0:
            processed_rows.append(
                {
                    "latitude": lat,
                    "longitude": lon,
                    "power_type": "DCFC",
                    "power_kw": 150.0,
                    "cost_per_kwh": cost_per_kwh,
                }
            )

    return pd.DataFrame(processed_rows)


def _parse_cost_per_kwh(pricing_str: str) -> float:
    """
    Parse cost per kWh from the ev_pricing string.

    Parameters:
    -----------
    pricing_str : str or None
        Pricing string from NREL API

    Returns:
    --------
    float
        Cost per kWh in dollars, or np.nan if free/unknown
    """
    try:
        import pandas as pd
    except ImportError as _:
        raise ImportError("Required libraries not installed. Please install pandas.")
    if pd.isna(pricing_str) or pricing_str is None:
        return 0.0

    pricing_str = str(pricing_str).strip()

    # Handle free cases
    if "free" in pricing_str.lower():
        return 0.0

    # Look for patterns like "$0.20 per kWh" or "$0.15/kWh"
    kwh_patterns = [
        r"\$(\d+\.?\d*)\s*per\s*kWh",
        r"\$(\d+\.?\d*)/kWh",
        r"\$(\d+\.?\d*)\s*kWh",
    ]

    for pattern in kwh_patterns:
        match = re.search(pattern, pricing_str, re.IGNORECASE)
        if match:
            return float(match.group(1))

    # Look for energy fee patterns like "$0.20/kWh Energy Fee"
    energy_fee_patterns = [
        r"\$(\d+\.?\d*)/kWh\s*Energy\s*Fee",
        r"\$(\d+\.?\d*)\s*per\s*kWh.*Energy.*Fee",
    ]

    for pattern in energy_fee_patterns:
        match = re.search(pattern, pricing_str, re.IGNORECASE)
        if match:
            return float(match.group(1))

    # Handle variable pricing - take the lower bound or average
    variable_patterns = [
        r"\$(\d+\.?\d*)-\$(\d+\.?\d*)/kWh\s*Variable\s*Energy\s*Fee",
        r"\$(\d+\.?\d*)-\$(\d+\.?\d*)\s*per\s*kWh",
    ]

    for pattern in variable_patterns:
        match = re.search(pattern, pricing_str, re.IGNORECASE)
        if match:
            low = float(match.group(1))
            high = float(match.group(2))
            return (low + high) / 2  # Return average of range

    # Handle cases with session fees + per kWh (extract just the kWh part)
    session_kwh_patterns = [
        r".*\+\s*\$(\d+\.?\d*)/kWh",
        r".*\+\s*\$(\d+\.?\d*)\s*per\s*kWh",
    ]

    for pattern in session_kwh_patterns:
        match = re.search(pattern, pricing_str, re.IGNORECASE)
        if match:
            return float(match.group(1))

    # If no pattern matches, return 0
    return 0


def states_intersecting_with_polygon(
    polygon: Union["Polygon", list["Polygon"]], states_gdf: "GeoDataFrame"
) -> "GeoDataFrame":
    """
    Find states that intersect with a given polygon.

    Parameters:
    -----------
    polygon : Polygon or List[Polygon]
        A shapely polygon or a list of polygons defining the area of interest.
    states_gdf : gpd.GeoDataFrame
        A GeoDataFrame containing state geometries. Must have a 'geometry' column.

    Returns:
    --------
    gpd.GeoDataFrame
        A GeoDataFrame containing the states that intersect with the given polygon(s).
    """
    try:
        from shapely.geometry import Polygon
    except ImportError as _:
        raise ImportError("Required libraries not installed. Please install geopandas.")
    if isinstance(polygon, Polygon):
        polygon = [polygon]  # Convert to list for consistency

    # Ensure the CRS matches between the states GeoDataFrame and the polygon
    states_crs = states_gdf.crs
    polygon = [poly.to_crs(states_crs) for poly in polygon]

    # Perform the spatial intersection
    intersecting_states = states_gdf[
        states_gdf.geometry.apply(
            lambda state_geom: any(state_geom.intersects(poly) for poly in polygon)
        )
    ]

    return intersecting_states


def get_states_from_polygon(
    polygon: "Polygon", states_gdf: "GeoDataFrame" = None
) -> list[str]:
    """
    Find all US states that intersect with the given polygon.

    Parameters:
    -----------
    polygon : shapely.geometry.Polygon
        The polygon to check against state boundaries
    states_gdf : geopandas.GeoDataFrame, optional
        Pre-loaded state boundaries GeoDataFrame. If None, will download from Census Bureau.

    Returns:
    --------
    List[str]
        List of two-letter state codes that intersect with the polygon
    """
    # Load state boundaries if not provided
    if states_gdf is None:
        states_gdf = load_us_state_boundaries()

    # Find intersecting states using the polygon directly
    intersecting_states = states_gdf[states_gdf.intersects(polygon.buffer(0))]

    # Return list of state codes
    state_list: list[str] = intersecting_states["state_code"].tolist()
    return state_list


def load_us_state_boundaries() -> "GeoDataFrame":
    """
    Load US state boundaries from the Census Bureau's cartographic boundary files.

    Returns:
    --------
    geopandas.GeoDataFrame
        GeoDataFrame containing state boundaries with STUSPS (state code) column
    """
    try:
        import geopandas as gpd
        import pandas as pd
    except ImportError as _:
        raise ImportError(
            "Required libraries not installed. Please install pandas and geopandas."
        )
    with importlib.resources.path(
        "nrel.routee.compass.resources", "us_states.csv.gz"
    ) as state_filepath:
        states_gdf = pd.read_csv(state_filepath)

    states_gdf = states_gdf.rename(columns={"STUSPS": "state_code"})
    states_gdf["geometry"] = gpd.GeoSeries.from_wkt(states_gdf["geometry"])
    states_gdf = gpd.GeoDataFrame(states_gdf, geometry="geometry", crs="EPSG:4326")
    states_gdf = states_gdf[["state_code", "geometry"]]

    return states_gdf


def download_ev_charging_stations_for_polygon(
    polygon: "Polygon", api_key: str = "DEMO_KEY", states_gdf: "GeoDataFrame" = None
) -> "GeoDataFrame":
    """
    Download EV charging stations for all states that intersect with a polygon.

    Parameters:
    -----------
    polygon : shapely.geometry.Polygon
        The polygon defining the area of interest
    api_key : str, optional
        NREL API key. Defaults to 'DEMO_KEY' which has rate limits.
    states_gdf : geopandas.GeoDataFrame, optional
        Pre-loaded state boundaries GeoDataFrame. If None, will download from Census Bureau.

    Returns:
    --------
    pd.DataFrame
        Combined DataFrame with charging stations from all intersecting states
    """
    try:
        import geopandas as gpd
        import pandas as pd
    except ImportError as _:
        raise ImportError(
            "Required libraries not installed. Please install pandas and geopandas."
        )
    # Get states that intersect with the polygon
    intersecting_states = get_states_from_polygon(polygon, states_gdf)

    if not intersecting_states:
        return pd.DataFrame(
            columns=["latitude", "longitude", "power_type", "power_kw", "cost_per_kwh"]
        )

    # Download charging stations for all intersecting states
    all_stations = []
    for state in intersecting_states:
        try:
            state_stations = download_ev_charging_stations(state, api_key)
            if not state_stations.empty:
                # Add state column for reference
                state_stations["state"] = state
                all_stations.append(state_stations)
        except RuntimeError as e:
            log.warning(f"Warning: Failed to download stations for state {state}: {e}")
            continue

    if not all_stations:
        return pd.DataFrame(
            columns=[
                "latitude",
                "longitude",
                "power_type",
                "power_kw",
                "cost_per_kwh",
                "state",
            ]
        )

    # Combine all stations
    combined_df = pd.concat(all_stations, ignore_index=True)

    # Optionally filter to only stations actually within the polygon
    # (since we downloaded by state, some stations might be outside the polygon)
    stations_gdf = gpd.GeoDataFrame(
        combined_df,
        geometry=gpd.points_from_xy(combined_df.longitude, combined_df.latitude),
        crs="EPSG:4326",
    )

    # Filter to stations within the polygon
    within_polygon = stations_gdf[stations_gdf.within(polygon)]

    # Create x and y columns from geometry
    within_polygon = within_polygon.assign(x=within_polygon.geometry.x)
    within_polygon = within_polygon.assign(y=within_polygon.geometry.y)

    return within_polygon
