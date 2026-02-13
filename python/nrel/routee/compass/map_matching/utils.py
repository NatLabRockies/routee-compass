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
    search_parameters: Optional[Dict[str, Any]] = None,
    output_format: Optional[str] = None,
    summary_ops: Optional[Dict[str, Any]] = None,
) -> CompassQuery:
    """
    Load a trace from a file and convert it into a map matching query.
    Automatically detects the file type based on the extension.

    Args:
        file: Path to the file (csv or gpx)
        x_col: Column name for longitude (only for csv)
        y_col: Column name for latitude (only for csv)
        search_parameters: Optional search configuration to override defaults
        output_format: The format to return the matched path in
        summary_ops: Operations to perform on the search state for the final summary

    Returns:
        A map matching query dictionary
    """
    path = pathlib.Path(file)
    ext = path.suffix.lower()
    if ext == ".csv":
        return load_trace_csv(
            path,
            x_col,
            y_col,
            search_parameters=search_parameters,
            output_format=output_format,
            summary_ops=summary_ops,
        )
    elif ext == ".gpx":
        return load_trace_gpx(
            path,
            search_parameters=search_parameters,
            output_format=output_format,
            summary_ops=summary_ops,
        )
    else:
        raise ValueError(f"Unsupported file extension: {ext}")


def load_trace_csv(
    file: Union[str, pathlib.Path],
    x_col: str = "longitude",
    y_col: str = "latitude",
    search_parameters: Optional[Dict[str, Any]] = None,
    output_format: Optional[str] = None,
    summary_ops: Optional[Dict[str, Any]] = None,
) -> CompassQuery:
    """
    Load a trace from a CSV file and convert it into a map matching query.

    Args:
        file: Path to the CSV file
        x_col: Column name for longitude
        y_col: Column name for latitude
        search_parameters: Optional search configuration to override defaults
        output_format: The format to return the matched path in
        summary_ops: Operations to perform on the search state for the final summary

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
        trace.append(point)

    query: CompassQuery = {"trace": trace}
    if search_parameters is not None:
        query["search_parameters"] = search_parameters
    if output_format is not None:
        query["output_format"] = output_format
    if summary_ops is not None:
        query["summary_ops"] = summary_ops

    return query


def load_trace_gpx(
    file: Union[str, pathlib.Path],
    search_parameters: Optional[Dict[str, Any]] = None,
    output_format: Optional[str] = None,
    summary_ops: Optional[Dict[str, Any]] = None,
) -> CompassQuery:
    """
    Load a trace from a GPX file and convert it into a map matching query.

    Args:
        file: Path to the GPX file
        search_parameters: Optional search configuration to override defaults
        output_format: The format to return the matched path in
        summary_ops: Operations to perform on the search state for the final summary

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
        trace.append(point)

    if not trace:
        # Try without namespace if none found (fallback for older/different GPX formats)
        for trkpt in root.findall(".//trkpt"):
            lat = float(trkpt.attrib["lat"])
            lon = float(trkpt.attrib["lon"])
            point = {"x": lon, "y": lat}
            trace.append(point)

    query: CompassQuery = {"trace": trace}
    if search_parameters is not None:
        query["search_parameters"] = search_parameters
    if output_format is not None:
        query["output_format"] = output_format
    if summary_ops is not None:
        query["summary_ops"] = summary_ops

    return query


def match_result_to_geopandas(
    results: Union[Result, Results],
) -> "GeoDataFrame":
    """
    Convert map matching results into a GeoPandas GeoDataFrame.
    Uses the 'matched_path' field of the result.

    Note:
        This function only works with results that have GeoJSON output format
        (output_format="json", which is the default). Results with other output
        formats (e.g., "edge_id", "wkt") will be skipped with a warning.

    Args:
        results: A single map matching result or a list of results

    Returns:
        A GeoPandas GeoDataFrame containing the matched path edges and their geometries
    """
    import warnings

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

        matched_path = result.get("matched_path")
        if matched_path is None:
            continue

        # Check if matched_path is a GeoJSON FeatureCollection
        if not (
            isinstance(matched_path, dict)
            and matched_path.get("type") == "FeatureCollection"
        ):
            warnings.warn(
                f"Result {i}: matched_path is not a GeoJSON FeatureCollection. "
                "This function only supports results with output_format='json'. "
                "Skipping this result.",
                UserWarning,
                stacklevel=2,
            )
            continue
        else:
            features = matched_path.get("features", [])
            for edge_idx, feature in enumerate(features):
                props = feature.get("properties", {})
                new_feature = {
                    "match_id": i,
                    "edge_index": edge_idx,
                    "edge_list_id": props.get("edge_list_id"),
                    "edge_id": props.get("edge_id"),
                }
                # Integrate state variables if they exist
                state = props.get("state")
                if isinstance(state, dict):
                    new_feature.update(state)
                geometry_data = feature.get("geometry")
                if geometry_data:
                    # Feature geometry is already a GeoJSON-like dict
                    if geometry_data.get("type") == "LineString":
                        coords = geometry_data.get("coordinates", [])
                        new_feature["geometry"] = LineString(coords)
                    else:
                        new_feature["geometry"] = None
                else:
                    new_feature["geometry"] = None

                all_features.append(new_feature)

    if not all_features:
        return gpd.GeoDataFrame()

    gdf = gpd.GeoDataFrame(all_features)
    gdf.crs = "EPSG:4326"
    return gdf
