from unittest import TestCase
import tempfile
import pathlib
from nrel.routee.compass.map_matching.utils import (
    load_trace_csv,
    load_trace_gpx,
    load_trace,
    match_result_to_geopandas,
)


class TestMapMatchingUtils(TestCase):
    def test_load_trace_csv(self) -> None:
        """Test loading a trace from a CSV file."""
        # Create a temporary CSV file
        with tempfile.NamedTemporaryFile(mode='w', suffix='.csv', delete=False) as f:
            f.write("longitude,latitude,timestamp\n")
            f.write("-104.9735321,39.7625164,0\n")
            f.write("-104.9740539,39.7629127,1\n")
            f.write("-104.9745757,39.7633090,2\n")
            csv_path = pathlib.Path(f.name)

        try:
            query = load_trace_csv(csv_path)
            
            self.assertIn("trace", query)
            self.assertEqual(len(query["trace"]), 3)
            
            # Check first point
            self.assertEqual(query["trace"][0]["x"], -104.9735321)
            self.assertEqual(query["trace"][0]["y"], 39.7625164)
            
            # Check that timestamp is not included by default
            self.assertNotIn("t", query["trace"][0])
        finally:
            csv_path.unlink()

    def test_load_trace_csv_with_timestamp(self) -> None:
        """Test loading a trace from a CSV file with timestamp."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.csv', delete=False) as f:
            f.write("longitude,latitude,timestamp\n")
            f.write("-104.9735321,39.7625164,0\n")
            f.write("-104.9740539,39.7629127,1\n")
            csv_path = pathlib.Path(f.name)

        try:
            query = load_trace_csv(csv_path, t_col="timestamp")
            
            self.assertIn("trace", query)
            self.assertEqual(len(query["trace"]), 2)
            
            # Check that timestamp is included
            self.assertIn("t", query["trace"][0])
            self.assertEqual(query["trace"][0]["t"], 0)
        finally:
            csv_path.unlink()

    def test_load_trace_csv_custom_columns(self) -> None:
        """Test loading a trace from a CSV file with custom column names."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.csv', delete=False) as f:
            f.write("lon,lat\n")
            f.write("-104.9735321,39.7625164\n")
            f.write("-104.9740539,39.7629127\n")
            csv_path = pathlib.Path(f.name)

        try:
            query = load_trace_csv(csv_path, x_col="lon", y_col="lat")
            
            self.assertIn("trace", query)
            self.assertEqual(len(query["trace"]), 2)
            self.assertEqual(query["trace"][0]["x"], -104.9735321)
            self.assertEqual(query["trace"][0]["y"], 39.7625164)
        finally:
            csv_path.unlink()

    def test_load_trace_gpx(self) -> None:
        """Test loading a trace from a GPX file."""
        # Create a temporary GPX file
        gpx_content = """<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test" xmlns="http://www.topografix.com/GPX/1/1">
  <trk>
    <trkseg>
      <trkpt lat="39.7625164" lon="-104.9735321">
        <time>2024-01-01T00:00:00Z</time>
      </trkpt>
      <trkpt lat="39.7629127" lon="-104.9740539">
        <time>2024-01-01T00:00:01Z</time>
      </trkpt>
      <trkpt lat="39.7633090" lon="-104.9745757">
        <time>2024-01-01T00:00:02Z</time>
      </trkpt>
    </trkseg>
  </trk>
</gpx>"""
        
        with tempfile.NamedTemporaryFile(mode='w', suffix='.gpx', delete=False) as f:
            f.write(gpx_content)
            gpx_path = pathlib.Path(f.name)

        try:
            query = load_trace_gpx(gpx_path)
            
            self.assertIn("trace", query)
            self.assertEqual(len(query["trace"]), 3)
            
            # Check first point
            self.assertEqual(query["trace"][0]["x"], -104.9735321)
            self.assertEqual(query["trace"][0]["y"], 39.7625164)
            self.assertIn("t", query["trace"][0])
            self.assertEqual(query["trace"][0]["t"], "2024-01-01T00:00:00Z")
        finally:
            gpx_path.unlink()

    def test_load_trace_auto_detect_csv(self) -> None:
        """Test automatic file type detection for CSV."""
        with tempfile.NamedTemporaryFile(mode='w', suffix='.csv', delete=False) as f:
            f.write("longitude,latitude\n")
            f.write("-104.9735321,39.7625164\n")
            csv_path = pathlib.Path(f.name)

        try:
            query = load_trace(csv_path)
            self.assertIn("trace", query)
            self.assertEqual(len(query["trace"]), 1)
        finally:
            csv_path.unlink()

    def test_load_trace_auto_detect_gpx(self) -> None:
        """Test automatic file type detection for GPX."""
        gpx_content = """<?xml version="1.0" encoding="UTF-8"?>
<gpx version="1.1" creator="test" xmlns="http://www.topografix.com/GPX/1/1">
  <trk>
    <trkseg>
      <trkpt lat="39.7625164" lon="-104.9735321"/>
    </trkseg>
  </trk>
</gpx>"""
        
        with tempfile.NamedTemporaryFile(mode='w', suffix='.gpx', delete=False) as f:
            f.write(gpx_content)
            gpx_path = pathlib.Path(f.name)

        try:
            query = load_trace(gpx_path)
            self.assertIn("trace", query)
            self.assertEqual(len(query["trace"]), 1)
        finally:
            gpx_path.unlink()

    def test_match_result_to_geopandas_single(self) -> None:
        """Test converting a single map matching result to GeoDataFrame."""
        result = {
            "point_matches": [
                {"edge_list_id": 0, "edge_id": 1, "distance": 5.5},
                {"edge_list_id": 0, "edge_id": 2, "distance": 3.2},
            ],
            "matched_path": [
                {
                    "edge_list_id": 0,
                    "edge_id": 1,
                    "geometry": {
                        "type": "LineString",
                        "coordinates": [[-104.9735321, 39.7625164], [-104.9740539, 39.7629127]]
                    }
                },
                {
                    "edge_list_id": 0,
                    "edge_id": 2,
                    "geometry": {
                        "type": "LineString",
                        "coordinates": [[-104.9740539, 39.7629127], [-104.9745757, 39.7633090]]
                    }
                },
            ]
        }

        gdf = match_result_to_geopandas(result)
        
        self.assertEqual(len(gdf), 2)
        self.assertEqual(gdf.iloc[0]["edge_list_id"], 0)
        self.assertEqual(gdf.iloc[0]["edge_id"], 1)
        self.assertEqual(gdf.iloc[0]["match_id"], 0)
        self.assertEqual(gdf.iloc[0]["edge_index"], 0)
        self.assertIsNotNone(gdf.iloc[0]["geometry"])
        self.assertEqual(gdf.crs, "EPSG:4326")

    def test_match_result_to_geopandas_multiple(self) -> None:
        """Test converting multiple map matching results to GeoDataFrame."""
        results = [
            {
                "matched_path": [
                    {"edge_list_id": 0, "edge_id": 1},
                ]
            },
            {
                "matched_path": [
                    {"edge_list_id": 0, "edge_id": 2},
                ]
            }
        ]

        gdf = match_result_to_geopandas(results)
        
        self.assertEqual(len(gdf), 2)
        self.assertEqual(gdf.iloc[0]["match_id"], 0)
        self.assertEqual(gdf.iloc[1]["match_id"], 1)

    def test_match_result_to_geopandas_no_geometry(self) -> None:
        """Test converting a map matching result without geometry."""
        result = {
            "matched_path": [
                {"edge_list_id": 0, "edge_id": 1},
                {"edge_list_id": 0, "edge_id": 2},
            ]
        }

        gdf = match_result_to_geopandas(result)
        
        self.assertEqual(len(gdf), 2)
        self.assertIsNone(gdf.iloc[0]["geometry"])
        self.assertIsNone(gdf.iloc[1]["geometry"])

    def test_match_result_to_geopandas_with_error(self) -> None:
        """Test that results with errors are skipped."""
        results = [
            {"error": "Some error"},
            {
                "matched_path": [
                    {"edge_list_id": 0, "edge_id": 1},
                ]
            }
        ]

        gdf = match_result_to_geopandas(results)
        
        # Only the second result should be included
        self.assertEqual(len(gdf), 1)
        self.assertEqual(gdf.iloc[0]["match_id"], 1)
