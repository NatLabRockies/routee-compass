from __future__ import annotations
import enum
from dataclasses import dataclass
from typing import Any, Callable, Dict, List, Optional, Union, TYPE_CHECKING
from pathlib import Path

import importlib.resources
import json
import logging
import shutil
import tomlkit


from nrel.routee.compass.io import utils
from nrel.routee.compass.io.utils import CACHE_DIR, add_grade_to_graph
from nrel.routee.compass.io.charging_station_ops import (
    download_ev_charging_stations_for_polygon,
)

if TYPE_CHECKING:
    import networkx
    import pandas as pd
    import geopandas

log = logging.getLogger(__name__)


HIGHWAY_TYPE = str
KM_PER_HR = float
HIGHWAY_SPEED_MAP = dict[HIGHWAY_TYPE, KM_PER_HR]

# Parameters annotated with this pass through OSMnx, then GeoPandas, then to Pandas,
# this is a best-effort annotation since the upstream doesn't really have one
AggFunc = Callable[[Any], Any]


@dataclass
class HookParameters:
    """
    Parameters passed to hooks registered with generate_compass_dataset.

    These parameters allow developers to access and modify the road network
    data before the dataset generation process completes.

    Attributes:
        output_directory (Path): The directory where the dataset files are being written.
        vertices (pd.DataFrame): A DataFrame containing the vertex (node) data
            of the road network, including coordinates and IDs.
        edges (geopandas.GeoDataFrame): A GeoDataFrame containing the edge (link)
            data, including geometries and attributes like distance and speed.
        graph (networkx.MultiDiGraph): The processed NetworkX graph object
            representing the road network topology and attributes.
    """

    output_directory: Path
    vertices: pd.DataFrame
    edges: geopandas.GeoDataFrame
    graph: networkx.MultiDiGraph


DatasetHook = Callable[[HookParameters], None]


class GeneratePipelinePhase(enum.Enum):
    GRAPH = 1
    CONFIG = 2
    POWERTRAIN = 3
    CHARGING_STATIONS = 4

    @classmethod
    def default(cls) -> List[GeneratePipelinePhase]:
        return [cls.GRAPH, cls.CONFIG, cls.POWERTRAIN]


def list_available_vehicle_models() -> List[str]:
    """
    Return the list of all available vehicle model names that can be used
    with the ``vehicle_models`` parameter of :func:`generate_compass_dataset`
    and :meth:`CompassApp.from_graph`.

    Each name corresponds to a vehicle configuration JSON shipped with the
    package (filename stem, e.g. ``"2017_CHEVROLET_Bolt"``).

    Returns:
        names: sorted list of available vehicle model name strings

    Example:
        >>> from nrel.routee.compass import list_available_vehicle_models
        >>> models = list_available_vehicle_models()
        >>> print(models[:3])
    """
    with importlib.resources.path(
        "nrel.routee.compass.resources", "vehicles"
    ) as vehicles_dir:
        return sorted(p.stem for p in vehicles_dir.glob("*.json"))


def generate_compass_dataset(
    g: networkx.MultiDiGraph,
    output_directory: Union[str, Path],
    hwy_speeds: Optional[HIGHWAY_SPEED_MAP] = None,
    fallback: Optional[float] = None,
    agg: Optional[AggFunc] = None,
    phases: List[GeneratePipelinePhase] = GeneratePipelinePhase.default(),
    raster_resolution_arc_seconds: Union[str, int] = 1,
    default_config: bool = True,
    requests_kwds: Optional[Dict[Any, Any]] = None,
    afdc_api_key: str = "DEMO_KEY",
    vehicle_models: Optional[List[str]] = None,
    hooks: Optional[List[DatasetHook]] = None,
) -> None:
    """
    Processes a graph downloaded via OSMNx, generating the set of input
    files required for running RouteE Compass.

    The input graph is assumed to be the direct output of an osmnx download.

    Args:
        g: OSMNx graph used to generate input files
        output_directory: Directory path to use for writing new Compass files.
        hwy_speeds: OSM highway types and values = typical speeds (km per
            hour) to assign to edges of that highway type for any edges missing
            speed data. Any edges with highway type not in `hwy_speeds` will be
            assigned the mean preexisting speed value of all edges of that highway
            type.
        fallback: Default speed value (km per hour) to assign to edges whose highway
            type did not appear in `hwy_speeds` and had no preexisting speed
            values on any edge.
        agg: Aggregation function to impute missing values from observed values.
            The default is numpy.mean, but you might also consider for example
            numpy.median, numpy.nanmedian, or your own custom function. Defaults to numpy.mean.
        phases (List[GeneratePipelinePhase]): of the overall generate pipeline, which phases of the pipeline to run. Defaults to all (["graph", "grade", "config", "powertrain"])
        raster_resolution_arc_seconds (str, optional): If grade is added, the resolution (in arc-seconds) of the tiles to download (either 1 or 1/3). Defaults to 1.
        default_config (bool, optional): If true, copy default configuration files into the output directory. Defaults to True.
        requests_kwds (Optional[Dict], optional): Keyword arguments to pass to the `requests` Python library for HTTP configuration. Defaults to None.
        afdc_api_key (str, optional): API key for the AFDC API to download EV charging stations. Defaults to "DEMO_KEY". See https://developer.nrel.gov/docs/transportation/alt-fuel-stations-v1/all/ for more information.
        vehicle_models (Optional[List[str]]): If provided, only download and
            configure the listed vehicle models (by name, e.g.
            ``["2017_CHEVROLET_Bolt", "2016_TOYOTA_Camry_4cyl_2WD"]``).
            Use :func:`list_available_vehicle_models` to see valid names.
            When ``None`` (the default) all available models are included.
        hooks: Optional list of callables that take a ``HookParameters`` object.
            These hooks will be called after the dataset has been generated
            and before the function returns.
    Example:
        >>> import osmnx as ox
        >>> g = ox.graph_from_place("Denver, Colorado, USA")
        >>> generate_compass_dataset(g, Path("denver_co"))
    """
    try:
        import osmnx as ox
        import numpy as np
        import pandas as pd
        import geopandas as gpd
        from shapely.geometry import box
        import requests
    except ImportError:
        raise ImportError("requires osmnx to be installed. Try 'pip install osmnx'")

    log.info(f"running pipeline import with phases: [{[p.name for p in phases]}]")
    output_directory = Path(output_directory)
    output_directory.mkdir(parents=True, exist_ok=True)

    # default aggregation is via numpy mean operation
    agg = agg if agg is not None else np.mean

    # pre-process the graph
    log.info("processing graph topology and speeds")
    g1 = ox.truncate.largest_component(g)
    g1 = ox.add_edge_speeds(g1, hwy_speeds=hwy_speeds, fallback=fallback, agg=agg)
    g1 = ox.add_edge_bearings(g1)

    if GeneratePipelinePhase.POWERTRAIN in phases:
        log.info("adding grade information")
        g1 = add_grade_to_graph(
            g1, resolution_arc_seconds=raster_resolution_arc_seconds
        )

    v, e = ox.graph_to_gdfs(g1)

    # process vertices
    log.info("processing vertices")
    v = v.reset_index(drop=False).rename(columns={"osmid": "vertex_uuid"})
    v["vertex_id"] = range(len(v))

    # process edges
    log.info("processing edges")
    lookup = v.set_index("vertex_uuid")

    def replace_id(vertex_uuid: pd.Index) -> pd.Series[int]:
        return lookup.loc[vertex_uuid].vertex_id

    e = e.reset_index(drop=False).rename(
        columns={
            "u": "src_vertex_uuid",
            "v": "dst_vertex_uuid",
            "osmid": "edge_uuid",
            "length": "distance",
        }
    )
    e = e[e["key"] == 0]  # take the first entry regardless of what it is (is this ok?)
    e["edge_id"] = range(len(e))
    e["src_vertex_id"] = e.src_vertex_uuid.apply(replace_id)
    e["dst_vertex_id"] = e.dst_vertex_uuid.apply(replace_id)

    if GeneratePipelinePhase.GRAPH in phases:
        #   vertex tables
        log.info("writing vertex files")
        v.to_csv(output_directory / "vertices-complete.csv.gz", index=False)
        v[["vertex_id", "vertex_uuid"]].to_csv(
            output_directory / "vertices-mapping.csv.gz", index=False
        )
        v[["vertex_uuid"]].to_csv(
            output_directory / "vertices-uuid-enumerated.txt.gz",
            index=False,
            header=False,
        )
        v[["vertex_id", "x", "y"]].to_csv(
            output_directory / "vertices-compass.csv.gz", index=False
        )

        #   edge tables (CSV)
        log.info("writing edge files")
        compass_cols = ["edge_id", "src_vertex_id", "dst_vertex_id", "distance"]
        e.to_csv(output_directory / "edges-complete.csv.gz", index=False)
        e[compass_cols].to_csv(output_directory / "edges-compass.csv.gz", index=False)
        e[["edge_id", "edge_uuid"]].to_csv(
            output_directory / "edges-mapping.csv.gz", index=False
        )

        #   edge tables (TXT)
        log.info("writing edge attribute files")
        e.edge_uuid.to_csv(
            output_directory / "edges-uuid-enumerated.txt.gz", index=False, header=False
        )
        np.savetxt(
            output_directory / "edges-geometries-enumerated.txt.gz",
            e.geometry,
            fmt="%s",
        )  # doesn't quote LINESTRINGS
        e.speed_kph.to_csv(
            output_directory / "edges-posted-speed-enumerated.txt.gz",
            index=False,
            header=False,
        )
        e.highway.to_csv(
            output_directory / "edges-road-class-enumerated.txt.gz",
            index=False,
            header=False,
        )

        headings = [utils.calculate_bearings(i) for i in e.geometry.values]
        headings_df = pd.DataFrame(
            headings, columns=["arrival_heading", "departure_heading"]
        )
        headings_df.to_csv(
            output_directory / "edges-headings-enumerated.csv.gz",
            index=False,
            compression="gzip",
        )

    if GeneratePipelinePhase.POWERTRAIN in phases:
        e.grade.to_csv(
            output_directory / "edges-grade-enumerated.txt.gz",
            index=False,
            header=False,
        )

    # COPY DEFAULT CONFIGURATION FILES
    if GeneratePipelinePhase.CONFIG in phases and default_config:
        log.info("copying default configuration TOML files")
        base_config_files = [
            "osm_default_distance.toml",
            "osm_default_speed.toml",
        ]
        if GeneratePipelinePhase.POWERTRAIN in phases:
            base_config_files.extend(
                [
                    "osm_default_energy.toml",
                    "osm_default_temperature.toml",
                ]
            )
        if GeneratePipelinePhase.CHARGING_STATIONS in phases:
            base_config_files.append("osm_default_charging.toml")
        for filename in base_config_files:
            with importlib.resources.path(
                "nrel.routee.compass.resources", filename
            ) as init_toml_path:
                with init_toml_path.open() as f:
                    init_toml: dict[str, Any] = tomlkit.load(f)

            # When a vehicle subset is requested, rewrite the
            # vehicle_input_files list in the energy traversal model
            # so the app only tries to load files that were downloaded.
            if vehicle_models is not None:
                _filter_vehicle_input_files(init_toml, vehicle_models)

            with open(output_directory / filename, "w") as f:
                f.write(tomlkit.dumps(init_toml))

    # DOWLOAD ROUTEE ENERGY MODEL CATALOG AND VEHICLE CONFIGS
    if GeneratePipelinePhase.POWERTRAIN in phases:
        log.info("downloading the default RouteE Powertrain models")
        model_output_directory = output_directory / "models"
        if not model_output_directory.exists():
            model_output_directory.mkdir(exist_ok=True)

        with importlib.resources.path(
            "nrel.routee.compass.resources.models", "download_links.json"
        ) as model_link_path:
            with model_link_path.open() as f:
                model_links = json.load(f)

            # Determine which model .bin files need to be downloaded.
            # When vehicle_models is set we resolve the required .bin names
            # from the vehicle JSON configs (handles PHEVs that reference
            # two separate models).  Otherwise download everything.
            if vehicle_models is not None:
                required_bin_names = _resolve_required_model_bins(vehicle_models)
                filtered_links = {
                    k: v for k, v in model_links.items() if k in required_bin_names
                }
            else:
                filtered_links = model_links

            for model_name, model_link in filtered_links.items():
                model_destination = model_output_directory / f"{model_name}.bin"

                cached_model_destination = CACHE_DIR / f"{model_name}.bin"
                if not cached_model_destination.exists():
                    kwds: Dict[Any, Any] = (
                        requests_kwds if requests_kwds is not None else {}
                    )
                    download_response = requests.get(model_link, **kwds)
                    download_response.raise_for_status()
                    with cached_model_destination.open("wb") as f:  # type: ignore
                        f.write(download_response.content)  # type: ignore

                shutil.copy(cached_model_destination, model_destination)

        log.info("copying vehicle configuration files")
        vehicle_output_directory = output_directory / "vehicles"
        if not vehicle_output_directory.exists():
            vehicle_output_directory.mkdir(exist_ok=True)

        with importlib.resources.path(
            "nrel.routee.compass.resources", "vehicles"
        ) as vehicles_dir:
            if vehicles_dir.is_dir():
                for vehicle_file in vehicles_dir.glob("*.json"):
                    if vehicle_models is None or vehicle_file.stem in vehicle_models:
                        shutil.copy(
                            vehicle_file,
                            vehicle_output_directory / vehicle_file.name,
                        )

    if GeneratePipelinePhase.CHARGING_STATIONS in phases:
        log.info("Downloading EV charging stations for the road network bounding box.")
        vertex_gdf = gpd.GeoDataFrame(
            v[["vertex_id", "x", "y"]].copy(),
            geometry=gpd.points_from_xy(v.x, v.y),
            crs="EPSG:4326",
        )

        vertex_bounds = vertex_gdf.total_bounds
        vertex_bbox = box(
            vertex_bounds[0],
            vertex_bounds[1],
            vertex_bounds[2],
            vertex_bounds[3],
        )

        charging_gdf = download_ev_charging_stations_for_polygon(
            vertex_bbox, api_key=afdc_api_key
        )

        if charging_gdf.empty:
            log.warning(
                "No charging stations found in the bounding box for the road network. "
                "Skipping charging station processing."
            )
            return

        out_df = charging_gdf[
            [
                "power_type",
                "power_kw",
                "cost_per_kwh",
                "x",
                "y",
            ]
        ]

        out_df.to_csv(
            output_directory / "charging-stations.csv.gz",
            index=False,
            compression="gzip",
        )

    # RUN HOOKS
    if hooks is not None:
        log.info(f"running {len(hooks)} dataset generation hooks")
        params = HookParameters(
            output_directory=output_directory,
            vertices=v,
            edges=e,
            graph=g1,
        )
        for hook in hooks:
            hook(params)


def _resolve_required_model_bins(vehicle_models: List[str]) -> set[str]:
    """
    Given a list of vehicle model names (JSON file stems), determine the set
    of ``.bin`` model names that need to be downloaded.

    For simple ICE/BEV vehicles, the bin name equals the ``model_input_file``
    stem in the JSON.  For PHEVs, there are two bins (Charge_Depleting and
    Charge_Sustaining) nested inside the JSON.
    """
    required: set[str] = set()
    with importlib.resources.path(
        "nrel.routee.compass.resources", "vehicles"
    ) as vehicles_dir:
        for name in vehicle_models:
            vehicle_file = vehicles_dir / f"{name}.json"
            if not vehicle_file.exists():
                available = sorted(p.stem for p in vehicles_dir.glob("*.json"))
                raise ValueError(
                    f"Vehicle model '{name}' not found. "
                    f"Use list_available_vehicle_models() to see valid names. "
                    f"Available: {available}"
                )
            with vehicle_file.open() as f:
                vehicle_cfg = json.load(f)

            vtype = vehicle_cfg.get("type", "")
            if vtype == "phev":
                # PHEV vehicles reference two separate model files
                for sub_key in ("charge_depleting", "charge_sustaining"):
                    sub = vehicle_cfg.get(sub_key, {})
                    model_path = sub.get("model_input_file", "")
                    if model_path:
                        required.add(Path(model_path).stem)
            else:
                model_path = vehicle_cfg.get("model_input_file", "")
                if model_path:
                    required.add(Path(model_path).stem)
    return required


def _filter_vehicle_input_files(
    toml_config: dict[str, Any], vehicle_models: List[str]
) -> None:
    """
    Walk through a parsed TOML config and rewrite any
    ``vehicle_input_files`` arrays so they only reference vehicle JSON
    files present in the *vehicle_models* list.
    """
    vehicle_set = set(vehicle_models)

    def _matches(vehicle_path: str) -> bool:
        """Return True if the vehicle path stem is in the requested set."""
        return Path(vehicle_path).stem in vehicle_set

    # The vehicle_input_files list lives inside [[search.traversal.models]]
    # entries with type = "energy".
    search = toml_config.get("search", {})
    traversal = search.get("traversal", {})
    models = traversal.get("models", [])
    for model in models:
        if model.get("type") == "energy" and "vehicle_input_files" in model:
            model["vehicle_input_files"] = [
                p for p in model["vehicle_input_files"] if _matches(p)
            ]
