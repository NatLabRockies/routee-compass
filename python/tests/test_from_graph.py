from pathlib import Path
from unittest import TestCase

from nrel.routee.compass.io.generate_dataset import GeneratePipelinePhase
import osmnx as ox
import pytest
from nrel.routee.compass.compass_app import CompassApp

FIXTURE_PATH = Path(__file__).parent / "fixtures" / "denver_sample.graphml.gz"


class TestFromGraph(TestCase):
    @pytest.mark.skip(reason="Awaiting Powertrain V2 integration.")
    def test_from_graph_denver(self) -> None:
        """
        rjf 2026-10-28: maybe we remove this test case? it requires about 2 minutes
        of downloading powertrain models and elevation rasters. the 2
        minute bump on CI is non-trivial. perhaps we could unify the fixture
        installations into a single pass shared by this and all other tests/notebooks
        run during CI?
        """
        graph = ox.load_graphml(FIXTURE_PATH)

        # Test building app from graph
        app = CompassApp.from_graph(graph)

        # Verify app can run (requires model_name for energy config)
        query = {
            "origin_x": -104.9903,
            "origin_y": 39.7511,
            "destination_x": -104.9930,
            "destination_y": 39.7485,
            "model_name": "2017_CHEVROLET_Bolt",
            "weights": {"trip_time": 0, "trip_distance": 0},
        }
        result = app.run(query)

        self.assertNotIn("error", result)
        self.assertIn("route", result)

    def test_from_graph_custom_config(self) -> None:
        graph = ox.load_graphml(FIXTURE_PATH)

        # Test with specific config
        app = CompassApp.from_graph(
            graph, 
            config_file="osm_default_speed.toml",
            phases=[
                GeneratePipelinePhase.GRAPH,
                GeneratePipelinePhase.CONFIG
            ]
        )

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
