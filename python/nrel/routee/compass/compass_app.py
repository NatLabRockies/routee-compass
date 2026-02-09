from __future__ import annotations

import logging
from tempfile import TemporaryDirectory

from pathlib import Path
from typing import Any, List, Optional, Union, Callable, TYPE_CHECKING, cast
from nrel.routee.compass.routee_compass_py import (
    CompassAppWrapper,
)
from nrel.routee.compass.io.generate_dataset import (
    GeneratePipelinePhase,
    generate_compass_dataset,
)

if TYPE_CHECKING:
    from shapely.geometry import Polygon, MultiPolygon
    from nrel.routee.compass.utils.type_alias import (
        Config,
        OSMNXQuery,
        CompassQuery,
        Result,
        Results,
    )
    import networkx as nx

import tomlkit
import json


log = logging.getLogger(__name__)


class CompassApp:
    """
    The CompassApp holds everything needed to run a route query.
    """

    _app: CompassAppWrapper

    def __init__(self, app: CompassAppWrapper, config: Config):
        self._app = app
        self._config = config

    @classmethod
    def get_constructor(cls) -> CompassAppWrapper:
        """
        Return the underlying constructor for the application.
        This allows a child class to inherit the CompassApp python class
        and implement its own rust based app constructor, while still using
        the original python methods.
        """
        return CompassAppWrapper

    @classmethod
    def from_config_file(
        cls,
        config_file: Union[str, Path],
        parallelism: Optional[int] = None,
    ) -> CompassApp:
        """
        Build a CompassApp from a config file

        Args:
            config_file: Path to the config file
            parallelism: optional number of threads to use for parallel
                query execution. Overrides the value in the config file.

        Returns:
            app: A CompassApp object

        Example:
            >>> from nrel.routee.compass import CompassApp
            >>> app = CompassApp.from_config_file("config.toml")
        """
        config_path = Path(config_file)
        if not config_path.is_file():
            raise ValueError(f"Config file {str(config_path)} does not exist")
        with open(config_path) as f:
            toml_config = tomlkit.load(f)

        return cls.from_dict(toml_config, config_path, parallelism=parallelism)

    @classmethod
    def _get_default_config_file(cls, phases: List[GeneratePipelinePhase]) -> str:
        """
        Internal helper to get the default config file name based on the phases.
        """
        if GeneratePipelinePhase.POWERTRAIN in phases:
            return "osm_default_energy.toml"
        else:
            return "osm_default_speed.toml"

    @classmethod
    def from_dict(
        cls,
        config: tomlkit.TOMLDocument,
        working_dir: Optional[Path] = None,
        parallelism: Optional[int] = None,
    ) -> CompassApp:
        """
        Build a CompassApp from a configuration object

        Args:
            config: Configuration dictionary
            working_dir: optional path to working directory
            parallelism: optional number of threads to use for parallel
                query execution. Overrides the value in the config.

        Returns:
            app: a CompassApp object

        Example:
            >>> from nrel.routee.compass import CompassApp
            >>> conf = { "parallelism": 2 }
            >>> app = CompassApp.from_config(conf)
        """
        if parallelism is not None:
            config["parallelism"] = parallelism
        path_str = str(working_dir.absolute()) if working_dir is not None else ""
        toml_string = tomlkit.dumps(config)
        app = cls.get_constructor()._from_config_toml_string(toml_string, path_str)
        return cls(app, config)

    @classmethod
    def from_graph(
        cls,
        graph: nx.MultiDiGraph,
        config_file: Optional[str] = None,
        cache_dir: Optional[Union[str, Path]] = None,
        hwy_speeds: Optional[dict[str, Any]] = None,
        fallback: Optional[float] = None,
        agg: Optional[Callable[[Any], Any]] = None,
        phases: List[GeneratePipelinePhase] = GeneratePipelinePhase.default(),
        raster_resolution_arc_seconds: Union[str, int] = 1,
        vehicle_models: Optional[List[str]] = None,
        parallelism: Optional[int] = None,
        overwrite: bool = False,
    ) -> CompassApp:
        """
        Build a CompassApp from a networkx graph.

        Args:
            graph: the networkx graph to build the CompassApp from.
                This is assumed to be the direct output of an osmnx download.
            config_file: optional name of the config file to load from the
                generated dataset. If not set, it will default to
                'osm_default_energy.toml' if the powertrain phase is included,
                otherwise 'osm_default_speed.toml'.
            cache_dir: optional path to save necessary files to build the
                CompassApp. If not set, TemporaryDirectory will be used
                instead. Defaults to None.
            hwy_speeds: OSM highway types and values = typical speeds (km
                per hour) to assign to edges of that highway type for any
                edges missing speed data. Any edges with highway type not
                in `hwy_speeds` will be assigned the mean preexisting
                speed value of all edges of that highway type.
                Defaults to None.
            fallback: Default speed value (km per hour) to assign to edges
                whose highway type did not appear in `hwy_speeds` and had
                no preexisting speed values on any edge.
                Defaults to None.
            agg: Aggregation function to impute missing values from
                observed values. The default is numpy.mean, but you might
                also consider for example numpy.median, numpy.nanmedian,
                or your own custom function. Defaults to numpy.mean.
            phases (List[GeneratePipelinePhase]): of the overall generate pipeline,
                which phases of the pipeline to run.
                Defaults to all (["graph", "grade", "config", "powertrain"])
            raster_resolution_arc_seconds: If grade is added, the
                resolution (in arc-seconds) of the tiles to download
                (either 1 or 1/3). Defaults to 1.
            vehicle_models: If provided, only download and configure
                the listed vehicle models (by name, e.g.
                ``["2017_CHEVROLET_Bolt", "2016_TOYOTA_Camry_4cyl_2WD"]``).
                Use :func:`list_available_vehicle_models` to see valid names.
                When ``None`` (the default) all available models are included.
            parallelism: optional number of threads to use for parallel
                query execution. Overrides the value in the config file.
            overwrite: if True, will overwrite any existing files in the
                cache_dir. Defaults to False.

        Returns:
            CompassApp: a CompassApp object
        """
        if cache_dir is None:
            temp_dir = TemporaryDirectory()
            cache_dir = Path(temp_dir.name)
        else:
            cache_dir = Path(cache_dir)

        if config_file is None:
            config_file = cls._get_default_config_file(phases)

        config_path = cache_dir / config_file

        if not overwrite and config_path.exists():
            log.info(f"Using existing dataset found in {cache_dir}")
        else:
            generate_compass_dataset(
                graph,
                output_directory=cache_dir,
                hwy_speeds=hwy_speeds,
                fallback=fallback,
                agg=agg,
                phases=phases,
                raster_resolution_arc_seconds=raster_resolution_arc_seconds,
                default_config=True,
                vehicle_models=vehicle_models,
            )

        if not config_path.exists():
            raise FileNotFoundError(
                f"Config file {config_file} not found in {cache_dir}. "
                "Make sure the requested phases generated this config."
            )

        return cls.from_config_file(config_path, parallelism=parallelism)

    @classmethod
    def from_place(
        cls,
        query: OSMNXQuery,
        network_type: str = "drive",
        parallelism: Optional[int] = None,
        cache_dir: Optional[Union[str, Path]] = None,
        overwrite: bool = False,
        **kwargs: Any,
    ) -> CompassApp:
        """
        Build a CompassApp from a place

        Args:
            query: the query or queries to geocode to get place boundary
                polygon(s)
            network_type: what type of street network. Default to drive
                List of options: ["all", "all_public", "bike", "drive", "drive_service", "walk"]
            parallelism: optional number of threads to use for parallel
                query execution. Overrides the value in the config file.
            cache_dir: optional path to save necessary files to build the
                CompassApp. If not set, TemporaryDirectory will be used
                instead. Defaults to None.
            overwrite: if True, will overwrite any existing files in the
                cache_dir. Defaults to False.
            **kwargs: additional arguments to pass to `from_graph` and `generate_compass_dataset`

        Returns:
            CompassApp: a CompassApp object

        Example:
            >>> from nrel.routee.compass import CompassApp
            >>> app = CompassApp.from_place("Denver, Colorado, USA")
        """
        if cache_dir is not None:
            cache_path = Path(cache_dir)
            config_file = kwargs.get("config_file")
            if config_file is None:
                phases = kwargs.get("phases", GeneratePipelinePhase.default())
                config_file = cls._get_default_config_file(phases)

            config_path = cache_path / config_file
            if not overwrite and config_path.exists():
                log.info(f"Using existing dataset found in {cache_dir}")
                return cls.from_config_file(config_path, parallelism=parallelism)

        try:
            import osmnx as ox
        except ImportError:
            raise ImportError("requires osmnx to be installed. Try 'pip install osmnx'")

        graph = ox.graph_from_place(query, network_type=network_type)
        return cls.from_graph(
            graph,
            parallelism=parallelism,
            cache_dir=cache_dir,
            overwrite=overwrite,
            **kwargs,
        )

    @classmethod
    def from_polygon(
        cls,
        polygon: Union["Polygon" | "MultiPolygon"],
        network_type: str = "drive",
        parallelism: Optional[int] = None,
        cache_dir: Optional[Union[str, Path]] = None,
        overwrite: bool = False,
        **kwargs: Any,
    ) -> CompassApp:
        """
        Build a CompassApp from a polygon

        Args:
            polygon: the shape to get network data within. coordinates
                should be in unprojected latitude-longitude degrees
            network_type: what type of street network. Default to drive
                List of options: ["all", "all_public", "bike", "drive", "drive_service", "walk"]
            parallelism: optional number of threads to use for parallel
                query execution. Overrides the value in the config file.
            cache_dir: optional path to save necessary files to build the
                CompassApp. If not set, TemporaryDirectory will be used
                instead. Defaults to None.
            overwrite: if True, will overwrite any existing files in the
                cache_dir. Defaults to False.
            **kwargs: additional arguments to pass to `from_graph` and `generate_compass_dataset`

        Returns:
            CompassApp: a CompassApp object

        Example:
            >>> from nrel.routee.compass import CompassApp
            >>> from shapely import geometry
            >>> p1 = geometry.Point(0,0)
            >>> p2 = geometry.Point(1,0)
            >>> p3 = geometry.Point(1,1)
            >>> p4 = geometry.Point(0,1)
            >>> pointList = [p1, p2, p3, p4]
            >>> poly = geometry.Polygon(pointList)
            >>> app = CompassApp.from_polygon(poly)
        """
        if cache_dir is not None:
            cache_path = Path(cache_dir)
            config_file = kwargs.get("config_file")
            if config_file is None:
                phases = kwargs.get("phases", GeneratePipelinePhase.default())
                config_file = cls._get_default_config_file(phases)

            config_path = cache_path / config_file
            if not overwrite and config_path.exists():
                log.info(f"Using existing dataset found in {cache_dir}")
                return cls.from_config_file(config_path, parallelism=parallelism)

        try:
            import osmnx as ox
        except ImportError:
            raise ImportError("requires osmnx to be installed. Try 'pip install osmnx'")

        graph = ox.graph_from_polygon(polygon, network_type=network_type)
        return cls.from_graph(
            graph,
            parallelism=parallelism,
            cache_dir=cache_dir,
            overwrite=overwrite,
            **kwargs,
        )

    def run(
        self,
        query: Union[CompassQuery, List[CompassQuery]],
        config: Optional[Config] = None,
    ) -> Union[Result, Results]:
        """
        Run a query (or multiple queries) against the CompassApp

        Args:
            query: A query or list of queries to run
            config: optional configuration

        Returns:
            results: A list of results (or a single result if a single query was passed)

        Example:
            >>> from nrel.routee.compass import CompassApp
            >>> app = CompassApp.from_config_file("config.toml")
            >>> query = {
                    "origin_name": "NREL",
                    "destination_name": "Comrade Brewing Company",
                    "origin_x": -105.1710052,
                    "origin_y": 39.7402804,
                    "destination_x": -104.9009913,
                    "destination_y": 39.6757025
                }

            >>> result = app.run(query)

        """
        if isinstance(query, dict):
            queries = [query]
            single_query = True
        elif isinstance(query, list):
            queries = query
            single_query = False
        else:
            raise ValueError(
                f"Query must be a dict or list of dicts, not {type(query)}"
            )

        queries_str = list(map(json.dumps, queries))
        config_str = json.dumps(config) if config is not None else None

        results_json: List[str] = self._app._run_queries(queries_str, config_str)

        results: Results = list(map(json.loads, results_json))
        if single_query and len(results) == 1:
            return results[0]
        return results

    def graph_edge_origin(self, edge_id: int) -> int:
        """
        get the origin vertex id for some edge

        Args:
            edge_id: the id of the edge

        Returns:
            vertex_id: the vertex id at the source of the edge
        """
        return cast(int, self._app.graph_edge_origin(edge_id))

    def graph_edge_destination(self, edge_id: int) -> int:
        """
        get the destination vertex id for some edge

        Args:
            edge_id: the id of the edge

        Returns:
            vertex_id: the vertex id at the destination of the edge
        """
        return cast(int, self._app.graph_edge_destination(edge_id))

    def graph_edge_distance(
        self, edge_id: int, distance_unit: Optional[str] = None
    ) -> float:
        """
        get the distance for some edge

        Args:
            edge_id: the id of the edge
            distance_unit: distance unit, by default meters

        Returns:
            dist: the distance covered by traversing the edge
        """
        return cast(float, self._app.graph_edge_distance(edge_id, distance_unit))

    def graph_get_out_edge_ids(self, vertex_id: int) -> List[int]:
        """
        get the list of edge ids that depart from some vertex

        Args:
            vertex_id: the id of the vertex

        Returns:
            edges: the edge ids of edges departing from this vertex
        """
        return cast(List[int], self._app.graph_get_out_edge_ids(vertex_id))

    def graph_get_in_edge_ids(self, vertex_id: int) -> List[int]:
        """
        get the list of edge ids that arrive from some vertex

        Args:
            vertex_id: the id of the vertex

        Returns:
            edges: the edge ids of edges arriving at this vertex
        """
        return cast(List[int], self._app.graph_get_in_edge_ids(vertex_id))

    def map_match(
        self,
        query: Union[CompassQuery, List[CompassQuery]],
    ) -> Union[Result, Results]:
        """
        Run a map matching query (or multiple queries) against the CompassApp

        Args:
            query: A query or list of queries to run

        Returns:
            results: A list of results (or a single result if a single query was passed)

        Example:
            >>> from nrel.routee.compass import CompassApp
            >>> app = CompassApp.from_config_file("config.toml")
            >>> query = {
                    "trace": [
                        {"x": -105.1710052, "y": 39.7402804},
                        {"x": -105.1710052, "y": 39.7402804}
                    ]
                }
            >>> result = app.map_match(query)
        """
        if isinstance(query, dict):
            queries = [query]
            single_query = True
        elif isinstance(query, list):
            queries = query
            single_query = False
        else:
            raise ValueError(
                f"Query must be a dict or list of dicts, not {type(query)}"
            )

        queries_str = list(map(json.dumps, queries))
        results_json: List[str] = self._app._map_match(queries_str)

        results: Results = list(map(json.loads, results_json))
        if single_query and len(results) == 1:
            return results[0]
        return results
