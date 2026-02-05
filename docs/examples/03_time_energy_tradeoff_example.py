"""
# Time Energy Tradeoff Example

In this example, we'll demonstrate how to use RouteE Compass to compare the tradeoffs between time and energy consumption.

This builds off the [Open Street Maps Example](01_open_street_maps_example) and assumes that we've already downloaded a road network so be sure to check that one out first.
"""

# %%
import seaborn as sns
import numpy as np
import matplotlib.pyplot as plt

from nrel.routee.compass import CompassApp
from nrel.routee.compass.io.convert_results import results_to_geopandas
# %%

"""
First, we'll load the application from the pre-built configuration file.
"""

app = CompassApp.from_config_file("denver_co/osm_default_energy.toml")
# %%
"""
## Build Time and Energy Weights

Next, we'll sweep a range of values for our time and energy weights such that the sum of the two is equal to 1.

Note that the weights are combined with the vehicle_rates to arrive at the final link cost that gets minimized in the path algorithm.
Because of this, the weights might be more sensitive at different scales, depending on the vehicle rates.
In this case, we're assuming that the time cost is $30/hour ($0.5/minute) and the energy costs are $0.12/kWh for electric and $3.00/gallon for gasoline (liquid).
Based on our own testing, the energy weights are more sensitive as they approach one, so we'll use a square root transformation to make the sweep more uniform.
"""
energy_weights = np.linspace(0, 1, 100)
energy_weights = energy_weights ** (1 / 4)
time_weights = 1 - energy_weights


def get_test_cases(energy_key: str):
    return [
        {
            "name": f"energy_{i}",
            "weights": {
                "trip_distance": 0,
                "trip_time": time_weights[i],
                energy_key: energy_weights[i],
            },
        }
        for i in range(len(energy_weights))
    ]


"""
Let's take a quick look at the weights we've generated.
"""
# %%
plt.plot(np.linspace(0, 1, 100), time_weights, label="Time Weight")
plt.plot(np.linspace(0, 1, 100), energy_weights, label="Energy Weight")
plt.legend()
plt.title("Time and Energy Weights")
# %%
"""
## Run Queries

Now that we have they weights let's perform a grid search over all of them to compare the tradeoffs between time and energy consumption for a 2017 Chevy Bolt and a 2016 Toyota Corolla.

First, the Chevy Bolt:
"""
# %%
bev_query = {
    "origin_x": -104.9256,
    "origin_y": 39.6638949,
    "destination_x": -104.8659653,
    "destination_y": 39.7867693,
    "model_name": "2017_CHEVROLET_Bolt",
    "vehicle_rates": {
        "trip_distance": {"type": "distance", "factor": 0.655, "unit": "miles"},
        "trip_time": {"type": "time", "factor": 20.0, "unit": "hours"},
        "trip_energy_electric": {"type": "energy", "factor": 0.12, "unit": "kwh"},
    },
    "grid_search": {
        "test_cases": get_test_cases("trip_energy_electric"),
    },
}

bev_results = app.run(bev_query)

"""
With the results, we can convert them into a geodataframe and plot the time vs energy consumption.
"""
bev_gdf = results_to_geopandas(bev_results)
bev_ax = sns.scatterplot(
    data=bev_gdf,
    x="route.traversal_summary.trip_time.value",
    y="route.traversal_summary.trip_energy_electric.value",
    hue="request.weights.trip_energy_electric",
)
bev_ax.set(
    title="2017 Chevy Bolt Time vs Energy",
    xlabel="Time (minutes)",
    ylabel="Electric Energy (kWh)",
)
bev_ax.legend(title="Electric Energy Weight")

"""
Above you can see a nice Pareto front showing the tradeoff between time and energy consumption.
As the energy weight approaches 1, the time increases and the energy consumption decreases to it's minimum.
Between 23 and 24 minutes of travel time, the energy consmption decreses significantly, whereas as we go from 24 to 28 minutes, we only see a small additional decrease in energy consumption.

Let's take a look at what those actual routes look like:
"""
# %%
bev_gdf.explore(column="route.traversal_summary.trip_energy_electric.value")

"""
Something that stands out is that the routes that have higher energy consumption use the highway to gain a lower travel time at the expense of increased energy consumption.
The routes that have lower energy consumption use the local roads to reduce the energy consumption at the expense of increased travel time.

Next, let's take a look at the 2016 Toyota Corolla:
"""
# %%
ice_query = {
    "origin_x": -104.9256,
    "origin_y": 39.6638949,
    "destination_x": -104.8659653,
    "destination_y": 39.7867693,
    "model_name": "2016_TOYOTA_Camry_4cyl_2WD",
    "vehicle_rates": {
        "trip_distance": {"type": "distance", "factor": 0.655, "unit": "miles"},
        "trip_time": {"type": "time", "factor": 20.0, "unit": "hours"},
        "trip_energy_liquid": {"type": "energy", "factor": 3.0, "unit": "gge"},
    },
    "grid_search": {
        "test_cases": get_test_cases("trip_energy_liquid"),
    },
}

ice_results = app.run(ice_query)

ice_gdf = results_to_geopandas(ice_results)

# ⛏️
ice_ax = sns.scatterplot(
    data=ice_gdf,
    x="route.traversal_summary.trip_time.value",
    y="route.traversal_summary.trip_energy_liquid.value",
    hue="request.weights.trip_energy_liquid",
)
ice_ax.set(
    title="2016 Toyota Corrola Time vs Energy",
    xlabel="Time (minutes)",
    ylabel="Gasoline Consumption (gallons)",
)
ice_ax.legend(title="Liquid Energy Weight")

"""
Here, we see a quite different tradeoff between time and energy consumption.
The Toyota Camry achieves the minimum energy consumption at around 24 minutes of travel time and doesn't exhibit the same sharp decrease in energy consumption as the Chevy Bolt.
At the same time, there are no alternative routes with significantly longer travel times, relative to those that were found for the Chevy Bolt.

Lastly, let's take a look at the routes for the Toyota Camry:
"""

# %%
ice_gdf.explore(column="route.traversal_summary.trip_energy_liquid.value")

"""
Here we notice, similarly to the Chevy Bolt, that the routes that minimize time and have larger energy consumption use the highway, while the routes that minimize energy consumption use the local roads.
But, there are much fewer local alternatives for the Toyota Camry than there were for the Chevy Bolt.
"""
# %%
