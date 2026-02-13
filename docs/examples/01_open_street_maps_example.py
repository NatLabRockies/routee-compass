"""
# OpenStreetMap Example

In this example, we download a road network from OSM using the OSMNx package, and then process the result, resulting in a RouteE Compass network dataset.

## requirements

To download an open street maps dataset, we'll need some extra dependnecies which are included with the conda distribution of this pacakge:

```console
conda create -n routee-compass -c conda-forge python=3.11 nrel.routee.compass
```
"""

# %%

import osmnx as ox

import json

from nrel.routee.compass import CompassApp
from nrel.routee.compass.io import generate_compass_dataset, results_to_geopandas
from nrel.routee.compass.io.generate_dataset import GeneratePipelinePhase
from nrel.routee.compass.plot import plot_route_folium, plot_routes_folium
# %%

"""
## Building RouteE Compass Dataset

### Get OSM graph

First, we need to get an OSM graph that we can use to convert into the format RouteE Compass expects.

In this example we will load in a road network that covers Golden, Colorado as a small example, but this workflow will work with any osmnx graph (osmnx provides [many graph download operations](https://osmnx.readthedocs.io/en/stable/user-reference.html#module-osmnx.graph)).
"""

# %%
g = ox.graph_from_place("Denver, Colorado, USA", network_type="drive")
# %%


"""
### Convert Graph to Compass Dataset

Now, we call the `generate_compass_dataset` function which will convert the osmnx graph into files that are compatible with RouteE Compass.

```{note}
In order to get the most accurate energy results from the routee-powertrain vehicle models, it's important to include road grade information since it plays a large factor in vehicle energy consumption.
That being said, adding grade can be a big lift computationally. In our case, we pull digital elevation model (DEM) raster files from USGS and then use osmnx to append elevation and grade to the graph. 
If the graph is large, this can take a while to download and could take up a lot of disk space.
So, we recommend that you include grade information in your graph but want to be clear about the requirements for doing so.
```
"""

# %%
pipeline_phases = [
    GeneratePipelinePhase.CONFIG,
    GeneratePipelinePhase.GRAPH,
    GeneratePipelinePhase.POWERTRAIN,
    GeneratePipelinePhase.CHARGING_STATIONS,
]
generate_compass_dataset(g, output_directory="denver_co", phases=pipeline_phases)
# %%


"""
This will parse the OSM graph and write the RouteE Compass files into a new folder "denver_co/". If you take a look in this directory, you'll notice some `.toml` files like: `osm_default_energy.toml`.
These are configurations for the compass application. Take a look [here](https://nrel.github.io/routee-compass/config.html) for more information about this file.

## Running

### Load Application

Now we can load the application from one of our config files.
We'll pick `osm_default_energy.toml` for computing energy optimal routes.
"""

# %%
app = CompassApp.from_config_file("denver_co/osm_default_energy.toml")
# %%


"""
###  Queries

With our application loaded we can start computing routes by passing queries to the app.
To demonstrate, we'll route between two locations in Denver, CO utilzing the grid search input plugin to run three separate searches.

The `model_name` is the vehicle we want to use for the route. If you look in the folder `denver_co/models` you'll see a collection of routee-powertrain models that can be used to compute the energy for your query.

The `vehicle_state_variable_rates` section defines rates to be applied to each component of the cost function. In this case we use the following costs:

 - 0.655 dollars per mile
 - 20 dollars per hour (or 0.333 dollars per minute)
 - 3 dollars per gallon of gas

The `grid_search` section defines our test cases.
Here, we have three cases: [`least_time`, `least_energy`, `least_cost`].
In the least_time and least_energy cases, we zero out all other variable contributions using the `state_variable_coefficients` which always get applied to each cost componenet.
In the least_cost case, we allow each cost component to contribute equally and the algorithm will minimize the resulting cost from all components being added together (after getting multiplied by the appropriate `vehicle_state_variable_rate`.
"""
# %%


query = [
    {
        "origin_x": -104.969307,
        "origin_y": 39.779021,
        "destination_x": -104.975360,
        "destination_y": 39.693005,
        "model_name": "2016_TOYOTA_Camry_4cyl_2WD",
        "vehicle_rates": {
            "trip_distance": {"type": "distance", "factor": 0.655, "unit": "miles"},
            "trip_time": {"type": "time", "factor": 20.0, "unit": "hours"},
            "trip_energy_liquid": {"type": "energy", "factor": 3.0, "unit": "gge"},
        },
        "grid_search": {
            "test_cases": [
                {
                    "name": "least_time",
                    "weights": {
                        "trip_distance": 0,
                        "trip_time": 1,
                        "trip_energy_liquid": 0,
                    },
                },
                {
                    "name": "least_energy",
                    "weights": {
                        "trip_distance": 0,
                        "trip_time": 0,
                        "trip_energy_liquid": 1,
                    },
                },
                {
                    "name": "least_cost",
                    "weights": {
                        "trip_distance": 1,
                        "trip_time": 1,
                        "trip_energy_liquid": 1,
                    },
                },
            ]
        },
    },
]
# %%


"""
Now, let's pass the query to the application.

```{note}
A query can be a single object, or, a list of objects.
If the input is a list of objects, the application will run these queries in parallel over the number of threads defined in the config file under the `paralellism` key (defaults to 2).
```
"""

# %%


results = app.run(query)
# %%

"""
## Analysis

The application returns the results as a list of python dictionaries.
Since we used the grid search to specify three separate searches, we should get three results back:
"""

# %%


for r in results:
    error = r.get("error")
    if error is not None:
        print(f"request had error: {error}")

assert len(results) == 3, f"expected 3 results, found {len(results)}"
# %%


"""
### Traversal and Cost Summaries

Since we have the `traversal` output plugin activated by default, we can take a look at the summary for each result under the `traversal_summary` key.
"""

# %%


def pretty_print(dict):
    print(json.dumps(dict, indent=4))


results_map = {r["request"]["name"]: r for r in results}
shortest_time_result = results_map["least_time"]
least_energy_result = results_map["least_energy"]
least_cost_result = results_map["least_cost"]


"""
Summary of route result for distance, time, and energy:
"""

# %%


pretty_print(shortest_time_result["route"]["traversal_summary"])

# %%

"""
And, if we need to know the units and/or the initial conditions for the search, we can look at the state model
"""

# %%


pretty_print(shortest_time_result["route"]["state_model"])
# %%


"""
The cost section shows the costs per unit assigned to the trip, in dollars.

This is based on the user assumptions assigned in the configuration which can be overriden in the route request query.
"""

# %%

pretty_print(shortest_time_result["route"]["cost"])
# %%


"""
The cost_model section includes details for how these costs were calculated.

The user can set different state variable coefficients in the query that are weighted against the vehicle state variable rates.

The algorithm will rely on the weighted costs while the cost summary will show the final costs without weight coefficients applied.
"""

# %%


pretty_print(shortest_time_result["route"]["cost_model"])
# %%


"""
Each response object contains this information. The least energy traversal and cost summary are below.
"""

# %%


pretty_print(least_energy_result["route"]["traversal_summary"])
pretty_print(least_energy_result["route"]["cost"])
# %%


"""
What becomes interesting is if we can compare our choices. Here's a quick comparison of the shortest time and least energy routes:
"""

# %%

dist_diff = (
    shortest_time_result["route"]["traversal_summary"]["trip_distance"]["value"]
    - least_energy_result["route"]["traversal_summary"]["trip_distance"]["value"]
)
time_diff = (
    shortest_time_result["route"]["traversal_summary"]["trip_time"]["value"]
    - least_energy_result["route"]["traversal_summary"]["trip_time"]["value"]
)
enrg_diff = (
    shortest_time_result["route"]["traversal_summary"]["trip_energy_liquid"]["value"]
    - least_energy_result["route"]["traversal_summary"]["trip_energy_liquid"]["value"]
)
cost_diff = (
    shortest_time_result["route"]["cost"]["total_cost"]
    - least_energy_result["route"]["cost"]["total_cost"]
)
dist_unit = shortest_time_result["route"]["state_model"]["trip_distance"]["output_unit"]
time_unit = shortest_time_result["route"]["state_model"]["trip_time"]["output_unit"]
enrg_unit = shortest_time_result["route"]["state_model"]["trip_energy_liquid"][
    "output_unit"
]
print(f" - distance: {dist_diff:.2f} {dist_unit} further with time-optimal")
print(f" - time: {-time_diff:.2f} {time_unit} longer with energy-optimal")
print(f" - energy: {enrg_diff:.2f} {enrg_unit} more with time-optimal")
print(f" - cost: ${cost_diff:.2f} more with time-optimal")
# %%


"""
In addition to the summary, the result also contains much more information.
Here's a list of all the different sections that get returned:
"""

# %%


def print_keys(d, indent=0):
    for k in sorted(d.keys()):
        print(f"{' ' * indent} - {k}")
        if isinstance(d[k], dict):
            print_keys(d[k], indent + 2)


print_keys(least_energy_result)

# %%
"""
We can also convert the results into a geodataframe:
"""

gdf = results_to_geopandas(least_energy_result)
gdf.head()
# %%


"""
### Plotting

We can plot the results to see the difference between the two routes.
"""

# %%


# %%


"""
We can use the `plot_route_folium` function to plot single routes, passing in the `line_kwargs` parameter to customize the folium linestring:
"""

# %%


m = plot_route_folium(
    shortest_time_result, line_kwargs={"color": "red", "tooltip": "Shortest Time"}
)
m = plot_route_folium(
    least_energy_result,
    line_kwargs={"color": "green", "tooltip": "Least Energy"},
    folium_map=m,
)
m = plot_route_folium(
    least_cost_result,
    line_kwargs={"color": "blue", "tooltip": "Least Cost"},
    folium_map=m,
)
m
# %%


"""
We can also use the plot_routes_folium function and pass in multiple results. The function will color the routes based on the `value_fn` which takes a single result as an argument. For example, we can tell it to color the routes based on the total energy usage.
"""

# %%


folium_map = plot_routes_folium(
    results,
    value_fn=lambda r: r["route"]["traversal_summary"]["trip_energy_liquid"]["value"],
    color_map="plasma",
)
folium_map
# %%


"""
And the `plot_routes_folium` can also accept an existing `folium_map` parameter. Let's query our application with different origin and destination places:
"""

# %%


query[0] = {
    **query[0],
    "origin_x": -105.081406,
    "origin_y": 39.667736,
    "destination_x": -104.95414,
    "destination_y": 39.65316,
}
new_results = app.run(query)


folium_map = plot_routes_folium(
    new_results,
    value_fn=lambda r: r["route"]["traversal_summary"]["trip_energy_liquid"]["value"],
    color_map="plasma",
    folium_map=folium_map,
)
folium_map

# %%
