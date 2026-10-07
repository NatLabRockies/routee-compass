# Running

There are a few different ways you can interact with RouteE Compass:

## Python

We provide python bindings for the core engine that allow you to run queries from within python.
After following the [installation instructions](installation), you can load an application and run queries like this:

```python
from nrel.routee.compass import CompassApp

app = CompassApp.from_config_file("path/to/config.toml")

query = {
    "origin_name": "NREL",
    "destination_name": "Comrade Brewing Company",
    "origin_x": -105.1710052,
    "origin_y": 39.7402804,
    "destination_x": -104.9009913,
    "destination_y": 39.6757025,
}

result = app.run(query)
```

For a more detailed example, head [here](examples/01_open_street_maps_example).

### Generate Vertex Elevations

The dataset generator can sample vertex elevations without downloading vehicle
models. With the `osm` dependencies installed, select `GRAPH` and `ELEVATION`:

```python
import osmnx as ox
from nrel.routee.compass.io import generate_compass_dataset
from nrel.routee.compass.io.generate_dataset import GeneratePipelinePhase

graph = ox.graph_from_place("Denver, Colorado, USA", network_type="drive")
generate_compass_dataset(
    graph,
    output_directory="denver_co",
    phases=[GeneratePipelinePhase.GRAPH, GeneratePipelinePhase.ELEVATION],
    raster_resolution_arc_seconds=1,
)
```

This produces `vertices-elevations-enumerated.txt.gz` alongside the graph files.
It is gzip-compressed text containing one elevation in **meters** per line,
without a header or index. The first line belongs to `vertex_id = 0`, the second
to `vertex_id = 1`, and so on. Only vertices retained in the largest connected
component are included. Elevations use the DEM's reference datum, not height
relative to the route origin. Pair this file with the graph and vertex mapping
from the same generation run; OSM node IDs are not row indices.

`POWERTRAIN` includes elevation sampling and this export automatically, along
with the existing edge-grade output and vehicle-model downloads. Selecting both
`ELEVATION` and `POWERTRAIN` samples elevations once. `ELEVATION` alone writes
only the elevation file; `GRAPH` alone does not sample or export elevations.
Add `CONFIG` to `GRAPH` and `ELEVATION` for the default distance and speed configs.
The compact `vertices-compass.csv.gz` schema remains unchanged.

Sampling reuses the existing USGS DEM tiles and local `cache/` directory. The
default resolution is 1 arc-second; use `"1/3"` for finer tiles with larger
downloads. Initial sampling needs network access and sufficient disk space;
cached tiles are reused. This source is intended for US networks, not global
coverage. If any retained vertex has missing or non-finite elevation, generation
fails before writing dataset files, reporting example vertex IDs. Check DEM
coverage and cached raster data rather than substituting zero. Valid zero and
negative elevations are retained.

The file prepares data for the combined elevation/grade traversal model
([#586](https://github.com/NatLabRockies/routee-compass/issues/586)); exporting it
does not activate a new traversal model or change the A* energy heuristic.

## Command line application

You can also just build the rust application and run it from the command line.
After following the [installation instructions](installation), you can run the application like this:

```bash
path/to/routee-compass/rust/target/release/routee-compass --config path/to/config.toml path/to/query.json
```

This will load the graph and then run the query (or queries) from your `query.json` file, outputing results to a file called `results.json` in the current working directory.

Logging verbosity can be controlled via the `RUST_LOG` environment variable:

```bash
RUST_LOG=DEBUG path/to/routee-compass/rust/target/release/compass-app --config path/to/config.toml path/to/query.json
```
