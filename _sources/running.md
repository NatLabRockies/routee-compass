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
