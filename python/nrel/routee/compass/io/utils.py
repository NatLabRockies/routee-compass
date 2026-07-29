from __future__ import annotations

import logging
import math
from enum import Enum
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import networkx
    import shapely

log = logging.getLogger(__name__)

CACHE_DIR = Path("cache")


class TileResolution(Enum):
    ONE_ARC_SECOND = 1
    ONE_THIRD_ARC_SECOND = 13

    @classmethod
    def from_string(self, string: str) -> TileResolution:
        if string.lower() in ["1", "one"]:
            return TileResolution.ONE_ARC_SECOND
        elif string.lower() in ["1/3", "one third"]:
            return TileResolution.ONE_THIRD_ARC_SECOND
        else:
            raise ValueError(
                f"invalid string {string} for tile resolution. Must be one of: 1, one, 1/3, one third"
            )

    @classmethod
    def from_int(self, int: int) -> TileResolution:
        if int == 1:
            return TileResolution.ONE_ARC_SECOND
        elif int == 13:
            return TileResolution.ONE_THIRD_ARC_SECOND
        else:
            raise ValueError(
                f"invalid int {int} for tile resolution. Must be one of: 1, 13"
            )


def get_usgs_tiles(lat_lon_pairs: list[tuple[float, float]]) -> list[str]:
    def tile_index(lat: float, lon: float) -> str:
        if lat < 0 or lon > 0:
            raise ValueError(
                f"USGS Tiles are not available for point ({lat}, {lon}). "
                "Consider re-running with `grade=False`."
            )

        lat_deg = int(lat) + 1
        lon_deg = abs(int(lon)) + 1

        return f"n{lat_deg:02}w{lon_deg:03}"

    tiles = set()
    for lat, lon in lat_lon_pairs:
        tile = tile_index(lat, lon)
        tiles.add(tile)

    return list(tiles)


def _build_download_link(
    tile: str, resolution: TileResolution = TileResolution.ONE_ARC_SECOND
) -> str:
    base_link_fragment = "https://prd-tnm.s3.amazonaws.com/StagedProducts/Elevation/"
    resolution_link_fragment = f"{resolution.value}/TIFF/current/{tile}/"
    filename = f"USGS_{resolution.value}_{tile}.tif"
    link = base_link_fragment + resolution_link_fragment + filename

    return link


def _download_tile(
    tile: str,
    output_dir: Path = CACHE_DIR,
    resolution: TileResolution = TileResolution.ONE_ARC_SECOND,
) -> Path:
    try:
        import requests
    except ImportError:
        raise ImportError(
            "requires requests to be installed. Try 'pip install requests'"
        )
    url = _build_download_link(tile, resolution)
    filename = url.split("/")[-1]
    destination = output_dir / filename
    if destination.is_file():
        log.info(f"{destination!s} already exists, skipping")
        return destination

    with requests.get(url, stream=True) as r:
        log.info(f"downloading {tile}")
        try:
            r.raise_for_status()
        except requests.exceptions.HTTPError as e:
            raise ValueError(
                f"Failed to download USGS tile {tile} from {url}. "
                "If this road network is outside of the US, consider re-running without "
                "GeneratePipelinePhase.GRADE in the `phases` argument."
            ) from e

        destination.parent.mkdir(exist_ok=True)

        # write to file in chunks
        with destination.open("wb") as f:
            for chunk in r.iter_content(chunk_size=8192):
                f.write(chunk)

    return destination


def add_grade_to_graph(
    g: networkx.MultiDiGraph,
    output_dir: Path = Path("cache"),
    resolution_arc_seconds: str | int = 1,
    api_key: str | None = None,
) -> networkx.MultiDiGraph:
    """
    Adds grade information to the edges of a graph.
    If using an api_key will try and download the grades from Google API, otherwise
    this will download the necessary elevation data from USGS as raster tiles and cache them in the output_dir.
    The resolution of the tiles can be specified with the resolution parameter.
    USGS has elevation data in increasing resolutions of: 1 arc-second and 1/3 arc-second
    Average tile file sizes for each resolution are about:

    * 1 arc-second: 50 MB
    * 1/3 arc-second: 350 MB

    Args:
        g (nx.MultiDiGraph): The networkx graph to add grades to.
        output_dir (Path, optional): The directory to cache the downloaded tiles in. Defaults to Path("cache").
        resolution_arc_seconds (str, optional): The resolution (in arc-seconds) of the tiles to download (either 1 or 1/3). Defaults to 1.
        api_key: The google API key to pull down grade information. If
            None will use USGS raster elevation tiles

    Returns:
        g: The graph with grade information added to the edges.

    Example:
        >>> import osmnx as ox
        >>> g = ox.graph_from_place("Denver, Colorado, USA")
        >>> g = add_grade_to_graph(g)
        >>> g2 = ox.graph_from_place("Denver, Colorado, USA")
        >>> g2 = add_grade_to_graph(g2, api_key=<api_key>)
    """
    try:
        import osmnx as ox
    except ImportError:
        raise ImportError("requires osmnx to be installed. Try 'pip install osmnx'")

    if api_key is None:
        node_gdf = ox.graph_to_gdfs(g, nodes=True, edges=False)

        all_points = [(t.y, t.x) for t in node_gdf.itertuples()]

        tiles = get_usgs_tiles(all_points)

        if isinstance(resolution_arc_seconds, int):
            resolution = TileResolution.from_int(resolution_arc_seconds)
        elif isinstance(resolution_arc_seconds, str):
            resolution = TileResolution.from_string(resolution_arc_seconds)
        else:
            raise ValueError(
                f"invalid type for resolution {resolution_arc_seconds}."
                "Must be one of: int, str"
            )

        files = []
        for tile in tiles:
            downloaded_file = _download_tile(
                tile, output_dir=output_dir, resolution=resolution
            )
            files.append(downloaded_file)

        if len(files) == 0:
            raise ValueError(
                "No USGS tiles were downloaded. "
                "If this road network is outside of the US, consider re-running without `grade` in your ."
            )
        elif len(files) == 1:
            filepath: Path | list[Path] = files[
                0
            ]  # if only one file, pass it directly
        else:
            filepath = files

        g = ox.add_node_elevations_raster(g, filepath)
    else:
        g = ox.add_node_elevations_google(g, api_key=api_key)
    g = ox.add_edge_grades(g)

    return g


def compass_heading(point1: tuple[float, float], point2: tuple[float, float]) -> float:
    lon1, lat1 = point1
    lon2, lat2 = point2

    lat1, lon1, lat2, lon2 = map(math.radians, [lat1, lon1, lat2, lon2])

    dlon = lon2 - lon1

    x = math.sin(dlon) * math.cos(lat2)
    y = math.cos(lat1) * math.sin(lat2) - (
        math.sin(lat1) * math.cos(lat2) * math.cos(dlon)
    )

    initial_bearing = math.atan2(x, y)

    initial_bearing = math.degrees(initial_bearing)
    compass_bearing = (initial_bearing + 360) % 360

    return compass_bearing


def calculate_bearings(geom: shapely.geometry.LineString) -> tuple[int, int]:
    if len(geom.coords) < 2:
        raise ValueError("Geometry must have at least two points")
    if len(geom.coords) == 2:
        # start and end heading is equal
        heading = int(compass_heading(geom.coords[0], geom.coords[1]))
        return (heading, heading)
    else:
        start_heading = int(compass_heading(geom.coords[0], geom.coords[1]))
        end_heading = int(compass_heading(geom.coords[-2], geom.coords[-1]))
        # returns headings as a list of tuples
        return (start_heading, end_heading)
