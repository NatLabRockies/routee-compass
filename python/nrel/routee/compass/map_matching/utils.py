from __future__ import annotations

import pathlib
from typing import Any, Dict, Optional, Union, TYPE_CHECKING
import xml.etree.ElementTree as ET

if TYPE_CHECKING:
    from geopandas import GeoDataFrame

from nrel.routee.compass.utils.type_alias import CompassQuery, Result, Results


def load_trace(
    file: Union[str, pathlib.Path],
    x_col: str = "longitude",
    y_col: str = "latitude",
    t_col: Optional[str] = None,
) -> CompassQuery:
    """
    Load a trace from a file and convert it into a map matching query.
    Automatically detects the file type based on the extension.

    Args:
        file: Path to the file (csv or gpx)
        x_col: Column name for longitude (only for csv)
        y_col: Column name for latitude (only for csv)
        t_col: Column name for timestamp (optional, only for csv)

    Returns:
        A map matching query dictionary
    """
    path = pathlib.Path(file)
    ext = path.suffix.lower()
    if ext == ".csv":
        return load_trace_csv(path, x_col, y_col, t_col)
    elif ext == ".gpx":
        return load_trace_gpx(path)
    else:
        raise ValueError(f"Unsupported file extension: {ext}")


def load_trace_csv(
    file: Union[str, pathlib.Path],
    x_col: str = "longitude",
    y_col: str = "latitude",
    t_col: Optional[str] = None,
) -> CompassQuery:
    """
    Load a trace from a CSV file and convert it into a map matching query.

    Args:
        file: Path to the CSV file
        x_col: Column name for longitude
        y_col: Column name for latitude
        t_col: Column name for timestamp (optional)

    Returns:
        A map matching query dictionary
    """
    try:
        import pandas as pd
    except ImportError:
        raise ImportError(
            "requires pandas to be installed. Try 'pip install \"nrel.routee.compass[osm]\"'"
        )

    df = pd.read_csv(file)
    trace = []
    for _, row in df.iterrows():
        point: Dict[str, Any] = {"x": float(row[x_col]), "y": float(row[y_col])}
        if t_col and t_col in row:
            point["t"] = row[t_col]
        trace.append(point)

    return {"trace": trace}


def load_trace_gpx(file: Union[str, pathlib.Path]) -> CompassQuery:
    """
    Load a trace from a GPX file and convert it into a map matching query.

    Args:
        file: Path to the GPX file

    Returns:
        A map matching query dictionary
    """
    tree = ET.parse(file)
    root = tree.getroot()

    # Handle GPX namespaces
    namespace = {"gpx": "http://www.topografix.com/GPX/1/1"}

    trace = []
    # Search for track points
    for trkpt in root.findall(".//gpx:trkpt", namespace):
        lat = float(trkpt.attrib["lat"])
        lon = float(trkpt.attrib["lon"])
        point: Dict[str, Any] = {"x": lon, "y": lat}

        time_elem = trkpt.find("gpx:time", namespace)
        if time_elem is not None:
            point["t"] = time_elem.text

        trace.append(point)

    if not trace:
        # Try without namespace if none found (fallback for older/different GPX formats)
        for trkpt in root.findall(".//trkpt"):
            lat = float(trkpt.attrib["lat"])
            lon = float(trkpt.attrib["lon"])
            point = {"x": lon, "y": lat}
            time_elem = trkpt.find("time")
            if time_elem is not None:
                point["t"] = time_elem.text
            trace.append(point)

    return {"trace": trace}


def match_result_to_geopandas(
    results: Union[Result, Results],
) -> "GeoDataFrame":
    """
    Convert map matching results into a GeoPandas GeoDataFrame.
    Uses the 'matched_path' field of the result.

    Args:
        results: A single map matching result or a list of results

    Returns:
        A GeoPandas GeoDataFrame containing the matched path edges and their geometries
    """
    try:
        import geopandas as gpd
        from shapely.geometry import LineString
    except ImportError:
        raise ImportError(
            "requires geopandas and shapely to be installed. Try 'pip install nrel.routee.compass[osm]'"
        )

    if isinstance(results, dict):
        results = [results]

    all_features = []
    for i, result in enumerate(results):
        if "error" in result:
            continue

        matched_path = result.get("matched_path", [])
        for edge_idx, edge in enumerate(matched_path):
            feature = {
                "match_id": i,
                "edge_index": edge_idx,
                "edge_list_id": edge.get("edge_list_id"),
                "edge_id": edge.get("edge_id"),
            }

            geometry_data = edge.get("geometry")
            if geometry_data:
                # Assuming geometry is a GeoJSON-like dict or already a LineString
                if isinstance(geometry_data, dict):
                    # Check if it's a LineString
                    if geometry_data.get("type") == "LineString":
                        coords = geometry_data.get("coordinates", [])
                        feature["geometry"] = LineString(coords)
                    else:
                        # Fallback for other geometry types if any
                        pass
                elif isinstance(geometry_data, list):
                    # Handle list of dicts with x/y keys or list of coordinate pairs
                    if geometry_data and isinstance(geometry_data[0], dict):
                        coords = [(point["x"], point["y"]) for point in geometry_data]
                        feature["geometry"] = LineString(coords)
                    else:
                        # List of [x, y] pairs or tuples
                        feature["geometry"] = LineString(geometry_data)
                else:
                    # Generic case if it's already a shapely object or similar
                    feature["geometry"] = geometry_data
            else:
                feature["geometry"] = None

            all_features.append(feature)

    if not all_features:
        return gpd.GeoDataFrame()

    gdf = gpd.GeoDataFrame(all_features)
    gdf.crs = "EPSG:4326"
    return gdf
