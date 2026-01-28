from unittest import TestCase
from nrel.routee.compass import package_root
from nrel.routee.compass.compass_app import CompassApp


class TestMapMatch(TestCase):
    def test_map_match_denver(self) -> None:
        app = CompassApp.from_config_file(
            package_root()
            / "resources"
            / "downtown_denver_example"
            / "map_matching.toml"
        )

        query = {
            "trace": [
                {"x": -104.9735321, "y": 39.7625164, "t": 0},
                {"x": -104.9740539, "y": 39.7629127, "t": 1}
            ]
        }

        result = app.map_match(query)
        
        if not isinstance(result, dict):
            self.fail(f"Result is not a dict: {result}")
        
        if "error" in result:
            self.fail(f"Map matching failed with error: {result['error']}")
        
        self.assertIn("point_matches", result)
        self.assertIn("matched_path", result)

    def test_map_match_with_search_parameters(self) -> None:
        app = CompassApp.from_config_file(
            package_root()
            / "resources"
            / "downtown_denver_example"
            / "map_matching.toml"
        )

        query = {
            "trace": [
                {"x": -104.9735321, "y": 39.7625164, "t": 0},
                {"x": -104.9740539, "y": 39.7629127, "t": 1}
            ],
            "search_parameters": {
                "some_dummy_param": "value"
            }
        }

        result = app.map_match(query)
        
        self.assertNotIn("error", result)
        self.assertIn("point_matches", result)
        self.assertIn("matched_path", result)
