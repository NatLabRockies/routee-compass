from nrel.routee.compass import package_root
from nrel.routee.compass.compass_app import CompassApp
from nrel.routee.compass.plot.plot_folium import plot_matched_path_folium
import unittest
from typing import Any


class TestMapMatchPlot(unittest.TestCase):
    def setUp(self) -> None:
        self.app = CompassApp.from_config_file(
            package_root()
            / "resources"
            / "downtown_denver_example"
            / "map_matching.toml"
        )
        self.query: dict[str, Any] = {
            "trace": [
                {"x": -104.9735321, "y": 39.7625164, "t": 0},
                {"x": -104.9740539, "y": 39.7629127, "t": 1},
            ]
        }

    def test_plot_with_geometry(self) -> None:
        query = self.query.copy()
        query["include_geometry"] = True
        result = self.app.map_match(query)
        # Should not raise error
        assert isinstance(result, dict)
        m = plot_matched_path_folium(result)
        self.assertIsNotNone(m)

    def test_plot_without_geometry_error(self) -> None:
        query = self.query.copy()
        query["include_geometry"] = False
        result = self.app.map_match(query)
        assert isinstance(result, dict)
        with self.assertRaisesRegex(ValueError, "is missing geometry"):
            plot_matched_path_folium(result)

    def test_missing_matched_path_error(self) -> None:
        with self.assertRaisesRegex(KeyError, "Could not find 'matched_path'"):
            plot_matched_path_folium({"some": "other", "result": "format"})


if __name__ == "__main__":
    unittest.main()
