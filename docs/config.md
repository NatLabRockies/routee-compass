# App Config

Each `CompassApp` instance is defined by a configuration toml file.
The configuration file specifies things like "Which traversal model should I use, and what are its parameters?" and "Which data sources should I use?".

If you follow the [open street maps example](examples/01_open_street_maps_example), the code will produce a few configuration files in the `golden_co/` folder. Let's take a look at the `osm_default_energy.toml` file.
We added some annotations to describe the different sections:

```toml
# how many threads should a CompassApp use to process queries?
[system]
parallelism = 2

[algorithm]
# classic best-first search algorithm, will use cost estimates to guide the search
# toward a path destination.
type = "a*"

# the parameters for the underlying road network graph
[graph]
# a file containing all the graph edges and their adjacencies.
edge_list.input_file = "edges-compass.csv.gz"
# # more than one edge list can be provided:
# edge_list = [
#   { input_file = "edges-compass.csv.gz" },
#   { input_file = "edges-transit.csv.gz" }
# ]
# a file containing all the graph verticies
vertex_list_input_file = "vertices-compass.csv.gz"
# if verbose is true, you'll see more information when loading the graph
verbose = true

[mapping]
# vertex or edge-oriented mapping
type = "edge"
# geometries used to build linestrings. should be as many entries as there are graph.edge_list entries.
geometry = { input_file = "edges-geometries-enumerated.txt.gz" }
# mapping threshold
tolerance.distance = 15.0
# mapping threshold distance unit
tolerance.unit = "meters"
# allow queries without destinations, for shortest path tree results
queries_without_destinations = true
# whether we match queries via "point", "vertex_id", or "edge_id" (or arrays of combinations)
matching_type = "point"

[search.traversal]
# the traversal section can either be a single model configuration, or, if "combined"
# is specified, it can be a collection of models listed at [[search.traversal.models]]
type = "combined"
[[search.traversal.models]]
# model distances in miles
type = "distance"
distance_unit = "miles"

[[search.traversal.models]]
type = "speed"
# the file that has speeds for each edge in the graph
speed_table_input_file = "edges-posted-speed-enumerated.txt.gz"
# the units of the values in the speed table
speed_unit = "kph"

[[search.traversal.models]]
# model time in minutes using the above distances and speeds
type = "time"
time_unit = "minutes"

[[search.traversal.models]]
type = "grade"
# the file that has grades for each edge in the graph
grade_table_input_file = "edges-grade-enumerated.txt.gz"
# the units of the grade table
grade_unit = "decimal"

[[search.traversal.models]]
type = "energy"

# here, we specify which vehicles to make available at query time
# if you wanted to add more models, you would make a new [[search.traversal.models.vehicles]] section.
[[search.traversal.models.vehicles]]
# the name of the model that can be passed in from a query as "model_name"
name = "2012_Ford_Focus"
# the type of the vehicle, currently either:
# - "ice" i.e. Internal Combustion Engine (ICE)
# - "bev" i.e. Battery Electric Vehicle (BEV)
# - "phev" i.e. Plug-in Hybrid Electric Vehicle (PHEV)
type = "ice"
# the file for the routee-powertrain model
model_input_file = "models/2012_Ford_Focus.bin"
# the units of what the routee-powertrain model expects speed to be in
speed_unit = "mph"
# the units of what the routee-powertrain model expects grade to be in
grade_unit = "decimal"
# the units of what the routee-powertrain model outputs for energy
energy_rate_unit = "gallons gasoline/mile"
# the "best case" energy rate for this particular vehicle (something like highway mpg) that's used in the a-star algorithm
ideal_energy_rate = 0.02857143
# A real world adjustment factor for things like temperature and auxillary loads
real_world_energy_adjustment = 1.166

# what underlying machine learn framework to use [smartcore | interpolate ]
# in this case we use a model that interpolates the underlying model type over a regular grid
[search.traversal.models.vehicles.model_type.interpolate]
underlying_model_type = "smartcore"
speed_lower_bound = 0
speed_upper_bound = 100
speed_bins = 101
grade_lower_bound = -0.2
grade_upper_bound = 0.2
grade_bins = 41

## The cost section defines how we translate the search state into a cost that is minimized by the algorithm

# The vehicle rates get applied to each component of the cost

# based on 65.5 cents per mile 2023 IRS mileage rate, $/mile
[cost.vehicle_rates.trip_distance]
type = "factor"
factor = 0.655

# based on $20/hr approximation of 2023 median hourly wages, $/second
[cost.vehicle_rates.trip_time]
type = "factor"
factor = 0.333336

# based on AAA regular unleaded gas prices sampled 12/21/2023
[cost.vehicle_rates.trip_energy_liquid]
type = "factor"
factor = 3.120

# based on $0.50/kWh approximation of DCFC charge rates, $/kWhtype = "factor"
[cost.vehicle_rates.trip_energy_electric]
type = "factor"
factor = 0.50

# Each cost component get multiplied by the corresponding vehicle weight.
# So, you could make time more important than distance by increasing the time weight.
[cost.weights]
trip_distance = 1
trip_time = 1
trip_energy_liquid = 1
trip_energy_electric = 1

# A turn delay model that assigns a time cost to each type of turn
[[search.traversal.models]]
type = "turn_delay"
edge_heading_input_file = "edges-headings-enumerated.csv.gz"
[search.traversal.models.turn_delay_model]
type = "tabular_discrete"
time_unit = "seconds"
[search.traversal.models.turn_delay_model.table]
no_turn = 0.0
slight_right = 0.5
right = 1.0
sharp_right = 1.5
slight_left = 1.0
left = 2.5
sharp_left = 3.5
u_turn = 9.5

# which plugins should be activated?
[plugin]
input_plugins = [
    # The grid search allows you to specify a "grid_search" key in the query and it will generate multiple queries from those parameters.
    { type = "grid_search" },
    # The load balancer estimates the runtime for each query and is used by CompassApp to best leverage parallelism.
    { type = "load_balancer", weight_heuristic = { type = "haversine" } },
]
output_plugins = [
    # The traversal plugin appends various items to the result.
    { type = "traversal", route = "geo_json", },
    # The uuid plugin adds a map specific id (like Open Street Maps Nodes) onto the compass verticies
    { type = "uuid", uuid_input_file = "vertices-uuid-enumerated.txt.gz" },
]
```

## Search Algorithm

The search algorithm used. The default is A*. Dijkstra's is also available. K-shortest path algorithms
exist as meta-algorithms, namely Yen's and Single-Via Paths.

### Single-Sourced Shortest Path (SSSP)

  - Dijkstra's Algorithm: see _Cormen, Thomas H., et al. Introduction to algorithms. MIT press, 2022._
  - A* Algorithm: see Wikipedia entry [A* Search Algorithm](https://en.wikipedia.org/wiki/A*_search_algorithm)

```toml
[algorithm]
# classic best-first search algorithm, will use cost estimates to guide the search
# toward a path destination.
type = "a*"

# # default behavior on search termination is "allow_tree_termination" which supports
# # reachability searches. if you do not want to allow partial search results, you
# # can choose "all_terminations_fail".
# termination_behavior = "all_terminations_fail"
```

To skip cost estimates and use a simple breadth-first search, choose "Dijkstra's":

```toml
[algorithm]
type = "dijkstras"
```

### K-Shortest Paths (KSP)

  - SVP: documented in Häcker, Christian, et al. "Most diverse near-shortest paths." Proceedings of the 29th International Conference on Advances in Geographic Information Systems. 2021.
  - Yen's: see Wikipedia [Yen's Algorithm](https://en.wikipedia.org/wiki/Yen%27s_algorithm)

To configure either ksp algorithm, you must set some additional parameters as well as 
choose the underlying search algorithm (a* or Dijkstra's):

```toml
# # classic k-shortest paths algorithm
type = "yens"
# type = "svp"
k = 5
similarity = { type = "distance_weighted_cosine_similarity", threshold = 0.1 }
termination = { type = "max_iteration", max = 10 }
[algorithm.underlying]
type = "a*"
```

#### KSP Parameters

_k_ is the target number of path alternatives in the result. It is possible for
less than _k_ to be returned, and this depends on the network and other KSP parameters.

Similarity function is optional and constraints path alternatives by their similarity
to the working solution set. In this example, both similarity functions will dismiss
paths that are greater than 10% similar.
```toml
# cheaper computation
similarity = { type = "edge_id_cosine_similarity", threshold = 0.1 }
# slightly more expensive but more correct
similarity = { type = "distance_weighted_cosine_similarity", threshold = 0.1 }

KSP termination criteria determines how long to run the meta-search. Given that a 
similarity function might dismiss a candidate path, it may require running a KSP
algorithm for many more iterations than _k_ to gather _k_ alternatives. 

```toml
# run it for k steps (default)
termination.type = "exact"
# run it for up to max steps until k is reached. max should be >= k.
termination = { type = "max_iteration", max = 10 }
# run it for up to (k*factor) steps until k is reached. useful if k varies between queries.
termination = { type = "factor", factor = 2 }
```

## Mapping Model

The mapping model deals with geospatial mappings from the road network graph. This may be represented using the graph vertices and drawing lines between coordinates, or, by loading LineString geometries from a file.

For example, if you specify your query origin and destination as lat/lon coordinates (i.e. `origin_x`, `origin_y`) we need a way to match this to the graph and then insert an `origin_vertex` or a `destination_vertex` into the query. Those two fields are what the application expects when conducting a search.

For vertex-oriented mapping, all fields are optional.

```toml
[mapping]
# # this matches incoming points to the nearest vertex in the graph.
# spatial_index_type = "vertex"
# # alternatively, builds the spatial index over edges in the graph.
# spatial_index_type = "edge"

# # if you don't have a file with linestring geometries, you can build
# # simple linestrings from the vertices of each edge list.
# geometry = { type = "from_vertices" } 
# geometry.type = "from_vertices"  # TOML shorthand for above

# # if you can bring your own, the resulting routes will have greater realism.
# geometry = { type = "from_linestrings", input_file = "edges-geometries-enumerated.txt.gz" }

# # optional query distance tolerance for map matching.
# tolerance.distance = 15.0
# tolerance.unit = "meters"

# # allow user to submit queries without destinations, such as when
# # shortest path trees are the desired result, not routes. true by default.
# queries_without_destinations = true

# # the default map input type is a combined strategy that attempts to
# # match by Point, otherwise expects the user to pass either a vertex ({origin|destination}_vertex)
# # or an edge ({origin|destination}_edge). a more restrictive strategy can be
# # specified here with a subset of these values or a single value such as "point".
# matching_type = ["point", "edge_id", "vertex_id"]
```

Edge-oriented mapping uses some additional (non-optional) line geometry input and builds a spatial lookup over those lines.

This model will map coordinates to `origin_edge` or a `destination_edge` into the query.

As opposed to vertex-oriented mapping, the edge-oriented will additionally apply any constraint model rules to any mapped edges, preventing mapping assignments that are invalid frontiers.

```toml
[mapping]
type = "edge"
# this can b
geometry = { input_file = "edges-geometries-enumerated.txt.gz" }

# # optional query distance tolerance for map matching.
# tolerance.distance = 15.0
# tolerance.unit = "meters"

# # allow user to submit queries without destinations, such as when
# # shortest path trees are the desired result, not routes. true by default.
# queries_without_destinations = true

# # the default map input type is a combined strategy that attempts to
# # match by Point, otherwise expects the user to pass either a vertex ({origin|destination}_vertex)
# # or an edge ({origin|destination}_edge). a more restrictive strategy can be
# # specified here with a subset of these values or a single value such as "point".
# matching_type = ["point", "edge_id", "vertex_id"]
```

## Traversal Models

Traversal models are what the application uses when computing a path through the graph.
The models can use things like road speed to compute the shortest time route or vehicle energy consumption to compute a route that uses the least energy.
Here are the default traversal models that come with the `CompassApp`:

### Distance

The distance traversal model is a very simple model that just uses distance for computing a route, producing the route that has the shortest distance.

```toml
[[search.traversal.models]]
type = "distance"
distance_unit = "miles"
```

### Speed

The speed table traversal model uses a speed lookup table to assign edge speeds.

```toml
[[search.traversal.models]]
type = "speed"
speed_table_input_file = "edges-posted-speed-enumerated.txt.gz"
speed_unit = "kph"
```

### Time

This simple model computes traversal time based on upstream distance and speed models.

```toml
[[search.traversal.models]]
type = "time"
time_unit = "minutes"
```

### Grade

Uses a lookup table to assign grade values.

```toml
[[search.traversal.models]]
type = "grade"
grade_input_file = "edges-grade-enumerated.txt.gz"
grade_unit = "decimal"
```

### Elevation

Assigns elevation gain and loss calculated from the grade value and distance.

```toml
[[search.traversal.models]]
type = "elevation"
distance_unit = "feet"
```

### Energy

The energy model computes energy (with a routee-powertrain vehicle model) and speed over an edge.

```toml
[[search.traversal.models]]
type = "energy"

# here, we specify which vehicles to make available at query time
# if you wanted to add more models, you would make a new [[traversal.models.vehicles]] section.
[[search.traversal.models.vehicles]]
# the name of the model that can be passed in from a query as "model_name"
name = "2012_Ford_Focus"
# the type of the vehicle, currently either:
# - "ice" i.e. Internal Combustion Engine (ICE)
# - "bev" i.e. Battery Electric Vehicle (BEV)
# - "phev" i.e. Plug-in Hybrid Electric Vehicle (PHEV)
type = "ice"
# the file for the routee-powertrain model
model_input_file = "models/2012_Ford_Focus.bin"
# the units of what the routee-powertrain model expects speed to be in
speed_unit = "mph"
# the units of what the routee-powertrain model expects grade to be in
grade_unit = "decimal"
# the units of what the routee-powertrain model outputs for energy
energy_rate_unit = "gallons gasoline/mile"
# the "best case" energy rate for this particular vehicle (something like highway mpg) that's used in the a-star algorithm
ideal_energy_rate = 0.02857143
# A real world adjustment factor for things like temperature and auxillary loads
real_world_energy_adjustment = 1.166

# what underlying machine learn framework to use [smartcore | interpolate ]
# in this case we use a model that interpolates the underlying model type over a regular grid
[search.traversal.models.vehicles.model_type.interpolate]
underlying_model_type = "smartcore"
speed_lower_bound = 0
speed_upper_bound = 100
speed_bins = 101
grade_lower_bound = -0.2
grade_upper_bound = 0.2
grade_bins = 41
```

## Plugins

Input and output plugins are used to modify the queries and the results respectively.
Both queries and results are valid json objects and so a plugin takes in a json object and returns a json object.

## Input Plugins

Here are the default input plugins that are provided:

### Grid Search

The grid search plugin allows you to specify a `grid_search` key in the query and it will generate multiple queries from those parameters. For example, if you had a query:

```json
{
  "origin_name": "Government Center Station",
  "destination_name": "Cannonball Creek Brewery",
  "origin_x": -105.200146,
  "origin_y": 39.72657,
  "destination_x": -105.234964,
  "destination_y": 39.768477,
  "model_name": "2016_TOYOTA_Camry_4cyl_2WD",
  "grid_search": {
    "test_cases": [
      {
        "name": "shortest_time",
        "weights": {
          "trip_distance": 0.0,
          "trip_time": 1.0,
          "trip_energy": 0.0
        }
      },
      {
        "name": "least_energy",
        "weights": {
          "trip_distance": 0.0,
          "trip_time": 0.0,
          "trip_energy": 1.0,
        }
      }
    ]
  }
}
```

The grid search plugin would take this single query and generate two queries that would be fed into the application:

```json
[
  {
    "origin_name": "Government Center Station",
    "destination_name": "Cannonball Creek Brewery",
    "origin_x": -105.200146,
    "origin_y": 39.72657,
    "destination_x": -105.234964,
    "destination_y": 39.768477,
    "model_name": "2016_TOYOTA_Camry_4cyl_2WD",
    "name": "shortest_time",
    "weights": {
      "trip_distance": 0.0,
      "trip_time": 1.0,
      "trip_energy": 0.0
    }
  },
  {
    "origin_name": "Government Center Station",
    "destination_name": "Cannonball Creek Brewery",
    "origin_x": -105.200146,
    "origin_y": 39.72657,
    "destination_x": -105.234964,
    "destination_y": 39.768477,
    "name": "least_energy",
    "model_name": "2016_TOYOTA_Camry_4cyl_2WD",
    "weights": {
      "trip_distance": 0.0,
      "trip_time": 0.0,
      "trip_energy": 1.0,
    }
  }
]
```

```toml
[[plugin.input_plugins]]
type = "grid_search"
```

### Load Balancer

The load balancer plugin estimates the runtime for each query. That information is used by `CompassApp` in order to best leverage parallelism.

For example, we have configured a parallelism of 2 and have 4 queries, but one query is a cross-country trip and will take a very long time to run.
With the load balancer plugin, Compass will identify this and bundle the three smaller queries together:

```
naive = [[long, short], [short, short]]
balanced = [[long], [short, short, short]]
```

```toml
[[plugin.input_plugins]]
type = "load_balancer"
# method for estimating query runtime, in this case haversine distance in kilometers.
# this heuristic only works for trips with origin/destination pairs.
weight_heuristic = { type = "haversine" }
```

if a user has fields on their queries that can be used directly or mapped to weight values, they may use
the custom weight heuristic. this numeric example expects a field `my_weight_value: float` on each query:

```toml
[[plugin.input_plugins]]
type = "load_balancer"
[plugin.input_plugins.weight_heuristic]
type = "custom"
[plugin.input_plugins.weight_heuristic.custom_weight_type]
type = "numeric"
column_name = "my_weight_value"
```

categorical fields are also supported by providing a mapping. this example expects a `mode` field
and uses values `[walk, bike, drive]` to map to weight values of 1, 10, and 100, for example
based on observed search sizes for each travel mode.

```toml
[[plugin.input_plugins]]
type = "load_balancer"
[plugin.input_plugins.weight_heuristic]
type = "custom"
[plugin.input_plugins.weight_heuristic.custom_weight_type]
type = "categorical"
column_name = "mode"
default = 1
mapping = { "walk" = 1, "bike" = 10, "drive" = 100 }
```

## Output Plugins

Here are the default output plugins that are provided:

### Traversal

A plugin that appends various items to the result. It leverages the mapping model for route and tree geometry generation.

```toml
[[plugin.output_plugins]]
type = "traversal"
route = "geo_json"
tree = "geo_json"
```

The `route` key will add route information to the result depending on the type.
The `tree` key will add search tree information to the result depending on the type (be aware that this could be very large for searches that span a large geographical distance).

Both the `route` and the `tree` key are optional and if omitted, the plugin will not append anything for it. In addition both keys can be specified in the following formats:

- "json": non-geometry output writing traversal metrics (cost, state) as JSON for a route or a tree
- "wkt": outputs a LINESTRING for a route, or a MULTILINESTRING for a tree
- "geo_json": annotated geometry data as a FeatureCollection of LineStrings with properties assigned from traversal metrics

### Eval

Evaluate arbitrary arithmetic expressions using data expected on the output.

Each expression takes inputs, defined as a mapping from [JSONPath](https://goessner.net/articles/JsonPath/) (on the right) to a variable name (on the left) to use. The variables, constants, infix operators and mathematical operations can be used in the `expr`. The result of the expression is stored on the `output` JSONPath. Under-the-hood, we use [fasteval](https://docs.rs/fasteval/latest/fasteval/) for expression evaluation; see the docs for any restrictions on variable naming.

For example, to estimate the cost per mile of a trip, we can take the `route.cost.total_cost` and divide it by the `route.traversal_summary.trip_time.value`. Here we assume the time is in minutes:

```toml
[[plugin.output_plugins]]
type = "eval"
expressions = [{
  inputs = {
    cost = "$.route.cost.total_cost",
    time = "$.route.traversal_summary.trip_time.value"
  },
  expr = "cost / (time / 60.0)",
  output = "$.route.cost.cost_per_hour"
}]
on_failure.type = "ignore"
```

Expressions can be chained to store incremental (and dependent) terms. Here, the recorded distance is assumed in miles:

```toml
[[plugin.output_plugins]]
type = "eval"
expressions = [{
  inputs = {
    cost = "$.route.cost.total_cost",
    time = "$.route.traversal_summary.trip_time.value"
  },
  expr = "cost / (time / 60.0)",
  output = "$.route.cost.cost_per_hour"
}, {
  inputs = {
    cost = "$.route.cost.total_cost",
    dist = "$.route.traversal_summary.trip_distance.value"
  },
  expr = "cost / (dist * 1.609)",
  output = "$.route.cost.cost_per_km"
}]
on_failure.type = "ignore"
```

#### Failure modes

If the evaluation fails, there are three behaviors:
```toml
# do nothing
on_failure.type = "ignore" 
# raise an error to the plugin
on_failure.type = "interrupt" 
# write the error to a location on the output
on_failure = { type = "record", path = "$.error" } 
```

#### Operations

The Eval plugin supports +,-,*,/ and the following math operations. Variable names used below provided as an example but not required, the functions accept any valid variable name or math constant.

Operation | Arguments | Example
--- | --- | ---
Sqrt  | x    | sqrt(x)  
Abs   | x    | abs(x)
Floor | x    | floor(x)
Ceil  | x    | ceil(x)
Round | x    | round(x)
Exp   | x    | exp(x)
Ln    | x    | ln(x)
Log2  | x    | log2(x)
Log10 | x    | log10(x)
Log   | b, x | log(b,x)
Sin   | x    | sin(x)
Cos   | x    | cos(x)
Tan   | x    | tan(x)
Asin  | x    | asin(x)
Acos  | x    | acos(x)
Atan  | x    | atan(x)
Atan2 | y, x | atan2(y,x)
Min   | a, b | min(a,b)
Max   | a, b | max(a,b)
Pow   | b, e | pow(b,e)

## System

The system section declares application-level parameters.

### Parallelism

Parallelism defines the number of threads to run searches on. Each thread grows a search tree or set of search trees in RAM. Each tree has a space complexity of _O(E * L)_ for _E_ graph edges and _L_ vertex labels. The correct parallelism depends on your system CPU and RAM resources along with your study area. 

Parallelism is an optional argument.

```toml
[system]
# default: 1
parallelism = 2
```

### Outputs

Compass can be configured to return results in memory or on disk using the `ResponsePersistencePolicy` and `ResponseOutputPolicy` respectively. These are both optional.

```toml
[system]
# the default. used in most Python Compass executions to allow Compass to return JSON
# result payloads into Python. Also supports Rust code integrations.
response_persistence_policy = "persist_response_in_memory"
# use on CLI runs of Compass to prevent memory leaks.
response_persistence_policy = "discard_response_from_memory"
```

Output to JSON:

```toml
[system.response_output_policy]
type = "file"
filepath = "result.json"
format = { type = "json", newline_delimited = false }

# append to an existing file (default)
write_mode = "append"
# # overwrite an existing file. do not use when running Compass in "chunking" mode.
# write_mode = "overwrite"
# # throw an error if the file exists. do not use when running Compass in "chunking" mode.
# write_mode = "error"

# # optionally set the "flush rate" to force write operations at some interval
# # here, for example, every 100 searches.
# file_flush_rate = 100
```

#### JSON files

Output to JSON in newline-delimited format:

```toml
[system.response_output_policy]
type = "file"
filepath = "result.jsonl"
format = { type = "json", newline_delimited = true }
```

or output a valid JSON value with `newline_delimited = false`.

#### CSV files

Output to CSV. This requires a mapping from JSON path to CSV column.

_note: when running Compass in "chunking" mode, always set sorted to `true` so that
subsequent chunks always share the same CSV header ordering._

```toml
[system.response_output_policy]
type = "file"
filepath = "result.csv"
[system.response_output_policy.format]
type = "csv"
sorted = true
[system.response_output_policy.format.mapping]
olon = "request.origin_x"
olat = "request.origin_y"
dlon = "request.destination_x"
dlat = "request.destination_y"
runtime = "search_app_runtime"
ram_mb = "search_result_size_mib"
error = { optional = "error" }
time = "route.traversal_summary.trip_time"
total_cost = "route.cost.total_cost"
```

#### Parquet files

Similar to CSV, this requires a mapping from JSON path to a parquet column. Provides concurrent write support, useful for large jobs.

_note: does not use Parquet partitioning._

```toml
[system.response_output_policy]
type = "file"
filepath = "result.parquet"
[system.response_output_policy.format]
type = "parquet"
sorted = true
[system.response_output_policy.format.mapping]
olon = "request.origin_x"
olat = "request.origin_y"
dlon = "request.destination_x"
dlat = "request.destination_y"
runtime = "search_app_runtime"
ram_mb = "search_result_size_mib"
error = { optional = "error" }
time = "route.traversal_summary.trip_time"
total_cost = "route.cost.total_cost"
```

#### Multiple file targets for CSV/JSON/JSONL

Writing to a pool of flat output files in CSV, JSON, or JSONL format is also possible. Similar to Parquet, concurrent writes are supported, which speeds up runtimes for large runs especially where each result contains a large amount of information.

```toml
[system.response_output_policy]
type = "directory"
path = "output/"
prefix = "data" # optional, defaults to "part"
# # sets the number of output file parts.
# # if not provided, the number of parallel threads used by Compass is used.
# concurrency = 8
format = { type = "json", newline_delimited = true }
# # other optional arguments:
# file_flush_rate = 50
# write_mode = "append" 
```

#### Multiple output configurations

Multiple output types are supported. Here, the complete output as JSON is 
preserved, while a CSV contains a high-level summary. An additional CSV file
contains only errors.

```toml
[system.response_output_policy]
type = "combined"

[[system.response_output_policy.policies]]
type = "file"
path = "result.jsonl"
format = { type = "json", newline_delimited = true }

[[system.response_output_policy.policies]]
type = "file"
path = "summary.csv"
[system.response_output_policy.policies.format]
sorted = true
[system.response_output_policy.policies.format.mapping]
olon = "request.origin_x"
olat = "request.origin_y"
dlon = "request.destination_x"
dlat = "request.destination_y"
runtime = "search_app_runtime"
ram_mb = "search_result_size_mib"
time = "route.traversal_summary.trip_time"
total_cost = "route.cost.total_cost"
# add other relevant summary columns here

[[system.response_output_policy.policies]]
type = "file"
path = "errors.csv"
[system.response_output_policy.policies.format]
type = "csv"
sorted = true
[system.response_output_policy.policies.format.mapping]
name = "request.name"  # any keys on queries are available here
olon = "request.origin_x"
olat = "request.origin_y"
dlon = "request.destination_x"
dlat = "request.destination_y"
error = { optional = "error" }
```
