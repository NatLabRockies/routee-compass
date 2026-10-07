import gzip
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import TestCase
from unittest.mock import call, patch

import networkx as nx
import numpy as np
import osmnx as ox
import pandas as pd
import pytest
import rasterio
from nrel.routee.compass.compass_app import CompassApp
from nrel.routee.compass.io import utils
from nrel.routee.compass.io.generate_dataset import (
    GeneratePipelinePhase,
    generate_compass_dataset,
)


class TestGenerateElevation(TestCase):
    def setUp(self) -> None:
        network = patch(
            "requests.get", side_effect=AssertionError("unexpected HTTP request")
        )
        network.start()
        self.addCleanup(network.stop)

    def graph(self) -> nx.MultiDiGraph:
        graph = nx.MultiDiGraph(crs="EPSG:4326")
        for node_id, longitude in [(90, -104.99), (10, -104.98), (50, -104.97)]:
            graph.add_node(node_id, x=longitude, y=39.75)
        for source, destination in [(90, 10), (10, 50)]:
            graph.add_edge(
                source,
                destination,
                osmid=source,
                highway="residential",
                maxspeed="30",
                length=100.0,
            )
        return graph

    def test_helper_raster_tiles(self) -> None:
        for tiles in [["n40w105"], ["n40w105", "n40w106"]]:
            with self.subTest(tiles=tiles):
                graph = self.graph()
                cache = Path("test-cache")
                files = [cache / f"{tile}.tif" for tile in tiles]
                with (
                    patch.object(utils, "get_usgs_tiles", return_value=tiles),
                    patch.object(
                        utils, "_download_tile", side_effect=files
                    ) as download,
                    patch.object(
                        ox, "add_node_elevations_raster", return_value=graph
                    ) as sample,
                    patch.object(ox, "add_edge_grades") as grades,
                ):
                    result = utils.add_elevation_to_graph(
                        graph, output_dir=cache, resolution_arc_seconds="1/3"
                    )
                self.assertIs(result, graph)
                self.assertEqual(
                    download.call_args_list,
                    [
                        call(
                            tile,
                            output_dir=cache,
                            resolution=utils.TileResolution.ONE_THIRD_ARC_SECOND,
                        )
                        for tile in tiles
                    ],
                )
                sample.assert_called_once_with(
                    graph, files[0] if len(files) == 1 else files
                )
                grades.assert_not_called()

    def test_helper_reuses_cached_tile(self) -> None:
        with TemporaryDirectory() as temporary:
            cache = Path(temporary)
            tile = cache / "USGS_1_n40w105.tif"
            tile.touch()
            self.assertEqual(utils._download_tile("n40w105", output_dir=cache), tile)

    def test_helper_missing_tiles(self) -> None:
        with (
            patch.object(utils, "get_usgs_tiles", return_value=[]),
            self.assertRaisesRegex(ValueError, "No USGS elevation tiles"),
        ):
            utils.add_elevation_to_graph(self.graph())
        with self.assertRaisesRegex(ValueError, "ELEVATION.*POWERTRAIN"):
            utils.get_usgs_tiles([(-10.0, -105.0)])

    def test_helper_google(self) -> None:
        graph = self.graph()
        with (
            patch.object(
                ox, "add_node_elevations_google", return_value=graph
            ) as sample,
            patch.object(utils, "_download_tile") as download,
            patch.object(ox, "add_edge_grades") as grades,
        ):
            self.assertIs(
                utils.add_elevation_to_graph(graph, api_key="test-key"), graph
            )
        sample.assert_called_once_with(graph, api_key="test-key")
        download.assert_not_called()
        grades.assert_not_called()

    def test_helper_grade_delegation(self) -> None:
        graph = self.graph()
        sampled = graph.copy()
        with (
            patch.object(
                utils, "add_elevation_to_graph", return_value=sampled
            ) as sample,
            patch.object(ox, "add_edge_grades", return_value=sampled) as grades,
        ):
            result = utils.add_grade_to_graph(
                graph,
                output_dir=Path("test-cache"),
                resolution_arc_seconds=13,
                api_key="test-key",
            )
        self.assertIs(result, sampled)
        sample.assert_called_once_with(
            graph,
            output_dir=Path("test-cache"),
            resolution_arc_seconds=13,
            api_key="test-key",
        )
        grades.assert_called_once_with(sampled)

    def sample_elevations(
        self, graph: nx.MultiDiGraph, filepath: Path
    ) -> nx.MultiDiGraph:
        nx.set_node_attributes(graph, {90: -2.5, 10: 0.0, 50: 17.125}, "elevation")
        return graph

    def test_phase_exports(self) -> None:
        phase_cases = [
            [GeneratePipelinePhase.ELEVATION],
            [GeneratePipelinePhase.GRAPH, GeneratePipelinePhase.ELEVATION],
            [GeneratePipelinePhase.GRAPH, GeneratePipelinePhase.POWERTRAIN],
            [
                GeneratePipelinePhase.GRAPH,
                GeneratePipelinePhase.ELEVATION,
                GeneratePipelinePhase.POWERTRAIN,
            ],
            None,
        ]
        for phases in phase_cases:
            with self.subTest(phases=phases), TemporaryDirectory() as temporary:
                graph = self.graph()
                graph.add_node(999, x=-104.0, y=40.0)
                output = Path(temporary)
                with (
                    patch.object(
                        utils, "_download_tile", return_value=Path("dem.tif")
                    ) as download,
                    patch.object(
                        ox,
                        "add_node_elevations_raster",
                        side_effect=self.sample_elevations,
                    ) as sample,
                    patch.object(
                        ox, "add_edge_grades", wraps=ox.add_edge_grades
                    ) as grades,
                ):
                    generate_compass_dataset(
                        graph,
                        output,
                        phases=phases,
                        raster_resolution_arc_seconds="1/3",
                        vehicle_models=[],
                    )
                sample.assert_called_once()
                self.assertEqual(list(sample.call_args.args[0].nodes), [90, 10, 50])
                download.assert_called_once_with(
                    "n40w105",
                    output_dir=utils.CACHE_DIR,
                    resolution=utils.TileResolution.ONE_THIRD_ARC_SECOND,
                )
                with gzip.open(
                    output / "vertices-elevations-enumerated.txt.gz", "rt"
                ) as stream:
                    elevations = [float(line) for line in stream]
                self.assertEqual(elevations, [-2.5, 0.0, 17.125])
                active_phases = (
                    phases if phases is not None else GeneratePipelinePhase.default()
                )
                if GeneratePipelinePhase.GRAPH in active_phases:
                    vertices = pd.read_csv(output / "vertices-compass.csv.gz")
                    self.assertEqual(list(vertices.columns), ["vertex_id", "x", "y"])
                    self.assertEqual(vertices.vertex_id.tolist(), [0, 1, 2])
                    mapping = pd.read_csv(output / "vertices-mapping.csv.gz")
                    self.assertEqual(mapping.vertex_uuid.tolist(), [90, 10, 50])
                    complete = pd.read_csv(output / "vertices-complete.csv.gz")
                    self.assertEqual(complete.elevation.tolist(), elevations)
                else:
                    self.assertEqual(
                        [path.name for path in output.iterdir()],
                        ["vertices-elevations-enumerated.txt.gz"],
                    )
                if GeneratePipelinePhase.POWERTRAIN in active_phases:
                    grades.assert_called_once()
                    edges = pd.read_csv(output / "edges-compass.csv.gz")
                    with gzip.open(
                        output / "edges-grade-enumerated.txt.gz", "rt"
                    ) as stream:
                        edge_grades = [float(line) for line in stream]
                    for edge, grade in zip(edges.itertuples(), edge_grades):
                        expected = (
                            elevations[edge.dst_vertex_id]
                            - elevations[edge.src_vertex_id]
                        ) / edge.distance
                        self.assertAlmostEqual(grade, expected)
                else:
                    grades.assert_not_called()
                    self.assertFalse(
                        (output / "edges-grade-enumerated.txt.gz").exists()
                    )
                    self.assertFalse((output / "models").exists())
                    self.assertFalse((output / "vehicles").exists())
                    self.assertFalse((output / "osm_default_energy.toml").exists())

    def test_local_raster_sampling_and_nodata(self) -> None:
        for last_elevation in [17.125, -9999.0]:
            with (
                self.subTest(last_elevation=last_elevation),
                TemporaryDirectory() as temporary,
            ):
                raster_path = Path(temporary) / "dem.tif"
                output = Path(temporary) / "dataset"
                values = np.array([[-2.5, 0.0, last_elevation]], dtype="float32")
                with rasterio.open(
                    raster_path,
                    "w",
                    driver="GTiff",
                    height=1,
                    width=3,
                    count=1,
                    dtype="float32",
                    crs="EPSG:4326",
                    nodata=-9999.0,
                    transform=rasterio.transform.from_origin(
                        -104.995, 39.755, 0.01, 0.01
                    ),
                ) as raster:
                    raster.write(values, 1)
                with (
                    patch.object(utils, "_download_tile", return_value=raster_path),
                    patch.object(
                        ox,
                        "add_node_elevations_raster",
                        side_effect=lambda graph, filepath: (
                            ox.elevation.add_node_elevations_raster(
                                graph, filepath, cpus=1
                            )
                        ),
                    ),
                ):
                    if last_elevation == -9999.0:
                        with self.assertRaisesRegex(
                            ValueError, "1 vertices.*vertex_uuid.*50"
                        ):
                            generate_compass_dataset(
                                self.graph(),
                                output,
                                phases=[GeneratePipelinePhase.ELEVATION],
                            )
                        self.assertEqual(list(output.iterdir()), [])
                    else:
                        generate_compass_dataset(
                            self.graph(),
                            output,
                            phases=[GeneratePipelinePhase.ELEVATION],
                        )
                        with gzip.open(
                            output / "vertices-elevations-enumerated.txt.gz", "rt"
                        ) as stream:
                            self.assertEqual(
                                [float(line) for line in stream], [-2.5, 0.0, 17.125]
                            )

    def test_graph_only_does_not_sample_or_export_elevation(self) -> None:
        for preexisting in [False, True]:
            with (
                self.subTest(preexisting=preexisting),
                TemporaryDirectory() as temporary,
            ):
                graph = self.graph()
                if preexisting:
                    self.sample_elevations(graph, Path("dem.tif"))
                with patch.object(ox, "add_node_elevations_raster") as sample:
                    generate_compass_dataset(
                        graph, temporary, phases=[GeneratePipelinePhase.GRAPH]
                    )
                sample.assert_not_called()
                self.assertFalse(
                    (Path(temporary) / "vertices-elevations-enumerated.txt.gz").exists()
                )

    def test_invalid_elevations_fail_before_writing(self) -> None:
        for phase in [
            GeneratePipelinePhase.ELEVATION,
            GeneratePipelinePhase.POWERTRAIN,
        ]:
            for invalid_value in [
                None,
                "invalid",
                float("nan"),
                float("inf"),
                -float("inf"),
            ]:
                for entire_column in [False, True]:
                    with (
                        self.subTest(
                            phase=phase,
                            value=invalid_value,
                            entire_column=entire_column,
                        ),
                        TemporaryDirectory() as temporary,
                    ):
                        graph = self.graph()
                        if not entire_column:
                            self.sample_elevations(graph, Path("dem.tif"))
                            graph.nodes[10]["elevation"] = invalid_value
                        with (
                            patch.object(
                                utils, "_download_tile", return_value=Path("dem.tif")
                            ),
                            patch.object(
                                ox,
                                "add_node_elevations_raster",
                                side_effect=lambda graph, filepath: graph,
                            ),
                            patch.object(ox, "add_edge_grades") as grades,
                            self.assertRaisesRegex(
                                ValueError,
                                "vertices have missing or non-finite elevation",
                            ) as error,
                        ):
                            generate_compass_dataset(
                                graph,
                                temporary,
                                phases=[
                                    GeneratePipelinePhase.GRAPH,
                                    GeneratePipelinePhase.CONFIG,
                                    phase,
                                ],
                            )
                        self.assertIn("vertex_id", str(error.exception))
                        self.assertIn("vertex_uuid", str(error.exception))
                        self.assertIn("USGS", str(error.exception))
                        self.assertTrue(
                            str(error.exception).startswith(
                                "3 vertices" if entire_column else "1 vertices"
                            )
                        )
                        grades.assert_not_called()
                        self.assertEqual(list(Path(temporary).iterdir()), [])


class TestFromGraph(TestCase):
    @pytest.mark.skip(reason="Awaiting Powertrain V2 integration.")
    def test_from_graph_denver(self) -> None:
        # Mini graph for testing (just a small area around a point)
        graph = ox.graph_from_point(
            (39.7511, -104.9903), dist=500, network_type="drive"
        )

        # Test building app from graph
        app = CompassApp.from_graph(graph)

        # Verify app can run (requires model_name for energy config)
        query = {
            "origin_x": -104.9903,
            "origin_y": 39.7511,
            "destination_x": -104.9930,
            "destination_y": 39.7485,
            "model_name": "2017_CHEVROLET_Bolt",
            "weights": {"trip_energy_electric": 1, "trip_time": 0, "trip_distance": 0},
        }
        result = app.run(query)

        self.assertNotIn("error", result)
        self.assertIn("route", result)

    def test_from_graph_custom_config(self) -> None:
        graph = ox.graph_from_point(
            (39.7511, -104.9903), dist=500, network_type="drive"
        )

        # Test with specific config
        app = CompassApp.from_graph(graph, config_file="osm_default_speed.toml")

        self.assertIsNotNone(app)

        # Verify it loaded speed config (requires weights)
        query = {
            "origin_x": -104.9903,
            "origin_y": 39.7511,
            "destination_x": -104.9930,
            "destination_y": 39.7485,
            "weights": {"trip_distance": 1, "trip_time": 0},
        }
        result = app.run(query)
        assert isinstance(result, dict)
        self.assertNotIn("error", result)
        self.assertIn("route", result)
        route = result["route"]
        assert isinstance(route, dict)
        self.assertNotIn("trip_energy_electric", route["traversal_summary"])
