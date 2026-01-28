import folium

from nrel.routee.compass.utils.type_alias import (
    Result as QueryResult,
    Results as QueryResults,
)

from typing import Any, Callable, Optional, Sequence, Tuple, Union
from nrel.routee.compass.plot.plot_utils import ColormapCircularIterator, rgba_to_hex

from nrel.routee.compass.utils.geometry import ROUTE_KEY, geometry_from_route

MATCHED_PATH_KEY = "matched_path"
GEOMETRY_KEY = "geometry"

DEFAULT_LINE_KWARGS = {
    "color": "blue",
    "weight": 10,
    "opacity": 0.8,
}


def result_dict_to_coords(result_dict: QueryResult) -> Sequence[Tuple[float, float]]:
    """
    Converts the CompassApp results to coords to be sent to the folium map.

    Args:
        result_dict (Dict[str, Any]): A result dictionary from a CompassApp query

    Returns:
        Sequence[(float, float)]: A sequence of latitude and longitude tuples.

    Example:
        >>> from nrel.routee.compass import CompassApp
        >>> from nrel.routee.compass.plot import result_dict_to_coords
        >>> app = CompassApp.from_config_file("config.toml")
        >>> query = {origin_x: -105.1710052, origin_y: 39.7402804, destination_x: -104.9009913, destination_y: 39.6757025}
        >>> result = app.run(query)
        >>> coords = result_dict_to_coords(result)

    """
    try:
        import shapely
    except ImportError:
        raise ImportError(
            "You need to install the shapely package to use this function"
        )

    if not isinstance(result_dict, dict):
        raise ValueError(f"Expected to get a dictionary but got a {type(result_dict)}")

    route = result_dict.get(ROUTE_KEY)
    if route is None:
        raise KeyError(
            f"Could not find '{ROUTE_KEY}' in result. "
            "Make sure the geometry output plugin is activated"
        )
    linestring = geometry_from_route(route)

    if isinstance(linestring, shapely.geometry.MultiLineString):
        coords = []
        for line in linestring.geoms:
            coords.extend([(lat, lon) for lon, lat in line.coords])
    else:
        coords = [(lat, lon) for lon, lat in linestring.coords]

    return coords


def _calculate_folium_args(fit_coords: Sequence[Tuple[float, float]]) -> dict[str, Any]:
    """
    Calculates where the center of the map and the bounds that the map
    should fit.

    Args:
        fit_coords (Sequence[Tuple[float, float]): The list of coords that needs to fit in the map

    Returns:
        dict: A dict with two keys. "location" contains the center of the map and "fit_bounds"
            represents a rectangle that covers all the coords.

    Example:
        >>> from nrel.routee.compass import CompassApp
        >>> from nrel.routee.compass.plot import _create_empty_folium_map
        >>> app = CompassApp.from_config_file("config.toml")
        >>> query = {origin_x: -105.1710052, origin_y: 39.7402804, destination_x: -104.9009913, destination_y: 39.6757025}
        >>> result = app.run(query)
        >>> coords = result_dict_to_coords(result)
        >>> folium_args = _calculate_folium_args(coords)

    """
    max_x = max(coord[0] for coord in fit_coords)
    min_x = min(coord[0] for coord in fit_coords)
    max_y = max(coord[1] for coord in fit_coords)
    min_y = min(coord[1] for coord in fit_coords)
    return {
        "location": ((max_x + min_x) / 2, (max_y + min_y) / 2),
        "fit_bounds": ([min_x, min_y], [max_x, max_y]),
    }


def _create_empty_folium_map(fit_coords: Sequence[Tuple[float, float]]) -> folium.Map:
    """
    Creates an empty folium.Map calculating the center and the fit_bounds
    using _calculate_folium_args.

    Args:
        fit_coords (Sequence[Tuple[float, float]): The list of coords that needs to fit in the map

    Returns:
        folium.Map: An empty folium map centered to fit all the coordinates

    Example:
        >>> from nrel.routee.compass import CompassApp
        >>> from nrel.routee.compass.plot import _create_empty_folium_map
        >>> app = CompassApp.from_config_file("config.toml")
        >>> query = {origin_x: -105.1710052, origin_y: 39.7402804, destination_x: -104.9009913, destination_y: 39.6757025}
        >>> result = app.run(query)
        >>> coords = result_dict_to_coords(result)
        >>> empty_folium_map = _create_empty_folium_map(coords)


    """
    try:
        import folium
    except ImportError:
        raise ImportError("You need to install the folium package to use this function")

    folium_args = _calculate_folium_args(fit_coords)
    folium_map = folium.Map(location=folium_args["location"], zoom_start=12)
    folium_map.fit_bounds(folium_args["fit_bounds"], max_zoom=12)
    return folium_map


def plot_route_folium(
    result_dict: QueryResult,
    line_kwargs: Optional[QueryResult] = None,
    folium_map: Optional[folium.Map] = None,
) -> folium.Map:
    """
    Plots a single route from a compass query on a folium map.

    Args:
        result_dict: A result dictionary from a CompassApp query
        line_kwargs: A dictionary of keyword arguments to pass to the folium Polyline
        folium_map: A existing folium map to plot the route on.

    Returns:
        folium_map: A folium map with the route plotted on it

    Example:
        >>> from nrel.routee.compass import CompassApp
        >>> from nrel.routee.compass.plot import plot_route_folium
        >>> app = CompassApp.from_config_file("config.toml")
        >>> query = {origin_x: -105.1710052, origin_y: 39.7402804, destination_x: -104.9009913, destination_y: 39.6757025}
        >>> result = app.run(query)
        >>> m = plot_route_folium(result)

    """
    coords = result_dict_to_coords(result_dict)

    return plot_coords_folium(coords, line_kwargs, folium_map)


def plot_coords_folium(
    coords: Sequence[Tuple[float, float]],
    line_kwargs: Optional[dict[str, Any]] = None,
    folium_map: Optional[folium.Map] = None,
) -> folium.Map:
    """
    Plots a sequence of pairs of latitude and longitude on a folium map as a route.

    Args:
        coords (Sequence[Tuple[float, float]]): A sequence of pairs of latitude and longitude
        line_kwargs (Optional[Dict[str, Any]], optional): A dictionary of keyword
            arguments to pass to the folium Polyline
        folium_map (folium.Map, optional): A existing folium map to plot the route on.
            Defaults to None.

    Returns:
        folium.Map: A folium map with the route plotted on it

    Example:
        >>> from nrel.routee.compass import CompassApp
        >>> from nrel.routee.compass.plot import plot_route_folium
        >>> app = CompassApp.from_config_file("config.toml")
        >>> query = {origin_x: -105.1710052, origin_y: 39.7402804, destination_x: -104.9009913, destination_y: 39.6757025}
        >>> result = app.run(query)
        >>> coords = result_dict_to_coords(result[0])
        >>> m = plot_coords_folium(coords)

    """
    try:
        import folium
    except ImportError:
        raise ImportError("You need to install the folium package to use this function")

    if folium_map is None:
        folium_map = _create_empty_folium_map(coords)

    kwargs = {**DEFAULT_LINE_KWARGS, **(line_kwargs or {})}

    folium.PolyLine(
        locations=coords,
        **kwargs,
    ).add_to(folium_map)

    start_icon = folium.Icon(color="green", icon="circle", prefix="fa")
    folium.Marker(
        location=coords[0],
        icon=start_icon,
        tooltip="Origin",
    ).add_to(folium_map)

    end_icon = folium.Icon(color="red", icon="circle", prefix="fa")
    folium.Marker(
        location=coords[-1],
        icon=end_icon,
        tooltip="Destination",
    ).add_to(folium_map)

    return folium_map


def plot_routes_folium(
    results: Union[QueryResult, QueryResults],
    value_fn: Callable[[QueryResult], Any] = lambda r: r["request"].get("name"),
    color_map: str = "viridis",
    folium_map: Optional[folium.Map] = None,
) -> folium.Map:
    """
    Plot multiple routes from a CompassApp query on a folium map

    Args:
        results: A result dictionary or list of result dictionaries from a CompassApp query
        value_fn: A function that takes a result dictionary and returns a value to use for coloring the routes.
            Defaults to lambda r: r["request"].get("name").
        color_map (str, optional): The name of the matplotlib colormap to use
            for coloring the routes. Defaults to "viridis".
        folium_map (folium.Map, optional): A existing folium map to plot the routes on.
            Defaults to None.

    Returns:
        folium_map: A folium map with the routes plotted on it

    Example:
        >>> from nrel.routee.compass import CompassApp
        >>> from nrel.routee.compass.plot import plot_results_folium
        >>> app = CompassApp.from_config_file("config.toml")
        >>> query = {origin_x: -105.1710052, origin_y: 39.7402804, destination_x: -104.9009913, destination_y: 39.6757025}
        >>> result = app.run(query)
        >>> m = plot_results_folium(result)

    """
    try:
        import matplotlib.pyplot as plt
        import matplotlib.colors as mcolors
    except ImportError:
        raise ImportError(
            "You need to install the matplotlib package to use this function"
        )
    try:
        import numpy as np
    except ImportError:
        raise ImportError("requires numpy to be installed. ")

    if isinstance(results, dict):
        results = [results]

    values = [value_fn(result) for result in results]

    cmap = plt.get_cmap(color_map)
    if all(isinstance(v, float) or isinstance(v, int) for v in values):
        norm = mcolors.Normalize(vmin=min(values), vmax=max(values))
        colors = [rgba_to_hex(cmap(norm(v))) for v in values]
    else:
        cmap_iter = ColormapCircularIterator(cmap, len(values))
        colors = [next(cmap_iter) for _ in values]

    results_coords = [result_dict_to_coords(result_dict) for result_dict in results]

    if folium_map is None:
        folium_map = _create_empty_folium_map(
            fit_coords=list(np.concatenate(results_coords))
        )

    for coords, value, route_color in zip(results_coords, values, colors):
        line_kwargs = {"color": route_color, "tooltip": f"{value}"}
        folium_map = plot_coords_folium(coords, line_kwargs, folium_map=folium_map)
    return folium_map


def matched_path_to_coords(
    matched_path: Sequence[dict[str, Any]],
) -> Sequence[Tuple[float, float]]:
    """
    Converts the matched path from a map matching query to coords to be sent to the folium map.

    Args:
        matched_path (Sequence[Dict[str, Any]]): A matched path from a map matching query

    Returns:
        Sequence[(float, float)]: A sequence of latitude and longitude tuples.
    """
    coords = []
    for edge in matched_path:
        geometry = edge.get(GEOMETRY_KEY)
        if geometry is None:
            raise ValueError(
                f"Edge {edge.get('edge_id')} is missing geometry. "
                "Make sure 'include_geometry' is set to True in the map matching request."
            )
        for point in geometry:
            coords.append((point["y"], point["x"]))
    return coords


def plot_matched_path_folium(
    result_dict: QueryResult,
    line_kwargs: Optional[QueryResult] = None,
    folium_map: Optional[folium.Map] = None,
) -> folium.Map:
    """
    Plots the matched path from a map matching query on a folium map.

    Args:
        result_dict: A result dictionary from a CompassApp map_match query
        line_kwargs: A dictionary of keyword arguments to pass to the folium Polyline
        folium_map: A existing folium map to plot the route on.

    Returns:
        folium_map: A folium map with the route plotted on it
    """
    matched_path = result_dict.get(MATCHED_PATH_KEY)
    if matched_path is None:
        raise KeyError(
            f"Could not find '{MATCHED_PATH_KEY}' in result. "
            "Make sure this is a result from a map matching query."
        )

    coords = matched_path_to_coords(matched_path)

    return plot_coords_folium(coords, line_kwargs, folium_map)
