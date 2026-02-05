from unittest import TestCase
from nrel.routee.compass.compass_app import CompassApp
import osmnx as ox


class TestFromGraph(TestCase):
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
