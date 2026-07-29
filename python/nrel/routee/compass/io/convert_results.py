from typing import TYPE_CHECKING, Optional, Union

from nrel.routee.compass.utils.geometry import geometry_from_route
from nrel.routee.compass.utils.type_alias import Result, Results

if TYPE_CHECKING:
    from geopandas import GeoDataFrame


def tree_result_to_geopandas(
    result: Result,
) -> Optional["GeoDataFrame"]:
    """convert the tree result to geopandas"""
    try:
        import geopandas as gpd
    except ImportError:
        raise ImportError(
            "requires geopandas to be installed. Try 'pip install nrel.routee.compass[osm]'"
        )
    if "error" in result:
        raise ValueError(f"Error in result: {result['error']}")

    tree = result.get("tree")
    if tree is None:
        return None
    elif isinstance(tree, list):
        raise NotImplementedError("Multiple trees are not yet supported")

    tree_gdf = gpd.GeoDataFrame.from_features(tree["features"])

    return tree_gdf


def route_result_to_geopandas(
    result: Result,
) -> Optional["GeoDataFrame"]:
    """convert the route result to geopandas"""
    try:
        import geopandas as gpd
        import pandas as pd
    except ImportError:
        raise ImportError(
            "requires geopandas to be installed. Try 'pip install nrel.routee.compass[osm]'"
        )
    if "error" in result:
        raise ValueError(f"Error in result: {result['error']}")

    route = result.get("route")
    if route is None:
        return None

    geometry = geometry_from_route(route)

    # use everything but the tree key since we are handling that separately
    result = {k: v for k, v in result.items() if k != "tree"}

    df = pd.json_normalize(result)
    df["geometry"] = geometry

    route_gdf = gpd.GeoDataFrame(df, geometry="geometry")
    route_gdf.crs = "EPSG:4326"

    # if the route was a geojson format, we can drop those columns
    if "route.path.type" in route_gdf.columns:
        route_gdf = route_gdf.drop(columns="route.path.type")
    if "route.path.features" in route_gdf.columns:
        route_gdf = route_gdf.drop(columns="route.path.features")

    return route_gdf


def results_to_geopandas(
    results: Result | Results,
) -> Union["GeoDataFrame", tuple["GeoDataFrame", "GeoDataFrame"]]:
    """convert the results to a geo dataframe"""
    try:
        import pandas as pd
    except ImportError:
        raise ImportError(
            "requires pandas to be installed. Try 'pip install nrel.routee.compass[osm]'"
        )
    if isinstance(results, dict):
        results = [results]

    route_constructor = []
    tree_constructor = []

    for i, result in enumerate(results):
        route_gdf = route_result_to_geopandas(result)
        if route_gdf is not None:
            route_gdf["route_id"] = i
            route_gdf = route_gdf.set_index("route_id")
            route_constructor.append(route_gdf)

        tree_gdf = tree_result_to_geopandas(result)
        if tree_gdf is not None:
            tree_gdf["tree_id"] = i
            tree_gdf = tree_gdf.set_index(["tree_id", "edge_id"])
            tree_constructor.append(tree_gdf)

    if len(route_constructor) == 0:
        full_route_gdf = None
    else:
        full_route_gdf = pd.concat(route_constructor)

    if len(tree_constructor) == 0:
        full_tree_gdf = None
    else:
        full_tree_gdf = pd.concat(tree_constructor)

    if full_route_gdf is not None and full_tree_gdf is not None:
        return full_route_gdf, full_tree_gdf
    elif full_route_gdf is not None:
        return full_route_gdf
    elif full_tree_gdf is not None:
        return full_tree_gdf
    else:
        raise ValueError("No route or tree results found in results")
