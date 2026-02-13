"""
# Different Powertrains Example

In this example, we'll showcase how you can run different powertrain technologies.

This builds off of the [Open Street Maps Example](01_open_street_maps_example) and assumes that we've already downloaded a road network so be sure to check that one out first.
"""


def main():
    # %%
    import seaborn as sns

    from nrel.routee.compass import CompassApp
    from nrel.routee.compass.io.convert_results import results_to_geopandas

    """
    ## Running

    ### Load Application

    Here, we'll load our application from the config file that was built in the [Open Street Maps Example](01_open_street_maps_example)
    """

    # %%

    app = CompassApp.from_config_file("denver_co/osm_default_energy.toml")

    """
    ## Queries

    RouteE Compass can execute search plans for internal combustion (ICE), plug-in hybrid (PHEV) and battery electric (BEV) powertrain types. 
    Each of these powertrain types are represented by RouteE Powertrain models.
    To see all the models we can use you can take a look at the folder `denver_co/models` and then reference the `traversal.vehicles` section of the `osm_default_energy.toml` configuration file to see how they are integrated and what name is expected as the `model_name` parameter in the query.
    Note that you can also train your own RouteE Powertrain models.
    Take a look at [this notebook](https://github.com/NREL/routee-powertrain/blob/main/docs/model_training-smartcore.ipynb) for more details.

    To demonstrate running other powertrain types, we'll use an ICE (2016_TOYOTA_Camry_4cyl_2WD), a BEV (2017_CHEVROLET_Bolt) and a PHEV (2016_CHEVROLET_Volt) model and create queries for each powertrain type.

    Notice that we can specify the `starting_soc_percent` for any vehicle that uses electric energy (BEV and PHEV).
    Also notice that we specify the energy weights for both "electric" and "liquid" energy that get applied to both energy sources.
    In the cases of the ICE and BEV, we only use one type of energy (liquid and electric respectively). 
    But, for the PHEV model, we use a combination of liquid and electric energy.
    By default, the PHEV will run using battery energy exclusively (starting at `starting_soc_percent`) and when the battery runs out, the vehicle will switch to it's hybrid mode, using primarily gasoline energy.
    """

    # %%

    query = [
        {
            "origin_x": -104.969307,
            "origin_y": 39.779021,
            "destination_x": -104.975360,
            "destination_y": 39.693005,
            "starting_soc_percent": 100,
            "vehicle_rates": {
                "trip_distance": {"type": "distance", "factor": 0.655, "unit": "miles"},
                "trip_time": {"type": "time", "factor": 20.0, "unit": "hours"},
            },
            "grid_search": {
                "_test_cases": [
                    {
                        "model_name": "2017_CHEVROLET_Bolt",
                        "vehicle_rates": {
                            "trip_energy_electric": {
                                "type": "energy",
                                "factor": 0.5,
                                "unit": "kwh",
                            }
                        },
                        "name": "least_time",
                        "weights": {
                            "trip_distance": 0,
                            "trip_time": 1,
                            "trip_energy_electric": 0,
                        },
                    },
                    {
                        "model_name": "2016_TOYOTA_Camry_4cyl_2WD",
                        "vehicle_rates": {
                            "trip_energy_liquid": {
                                "type": "energy",
                                "factor": 3.1,
                                "unit": "gge",
                            },
                        },
                        "name": "least_time",
                        "weights": {
                            "trip_distance": 0,
                            "trip_time": 1,
                            "trip_energy_liquid": 0,
                        },
                    },
                    {
                        "model_name": "2016_CHEVROLET_Volt",
                        "vehicle_rates": {
                            "trip_energy_electric": {
                                "type": "energy",
                                "factor": 0.5,
                                "unit": "kwh",
                            }
                        },
                        "name": "least_time",
                        "weights": {
                            "trip_distance": 0,
                            "trip_time": 1,
                            "trip_energy_liquid": 0,
                            "trip_energy_electric": 0,
                        },
                    },
                    {
                        "model_name": "2017_CHEVROLET_Bolt",
                        "vehicle_rates": {
                            "trip_energy_electric": {
                                "type": "energy",
                                "factor": 0.5,
                                "unit": "kwh",
                            }
                        },
                        "name": "least_energy",
                        "weights": {
                            "trip_distance": 0,
                            "trip_time": 0,
                            "trip_energy_electric": 1,
                        },
                    },
                    {
                        "model_name": "2016_TOYOTA_Camry_4cyl_2WD",
                        "vehicle_rates": {
                            "trip_energy_liquid": {
                                "type": "energy",
                                "factor": 3.1,
                                "unit": "gge",
                            },
                        },
                        "name": "least_energy",
                        "weights": {
                            "trip_distance": 0,
                            "trip_time": 0,
                            "trip_energy_liquid": 1,
                        },
                    },
                    {
                        "model_name": "2016_CHEVROLET_Volt",
                        "vehicle_rates": {
                            "trip_energy_electric": {
                                "type": "energy",
                                "factor": 0.5,
                                "unit": "kwh",
                            }
                        },
                        "name": "least_energy",
                        "weights": {
                            "trip_distance": 0,
                            "trip_time": 0,
                            "trip_energy_liquid": 1,
                            "trip_energy_electric": 1,
                        },
                    },
                ],
            },
        },
    ]

    # %%

    result = app.run(query)

    """
    First, let's review the state model to see what energy units we're using.
    """

    # %%

    result[0]["route"]["state_model"]

    """
    Now we can take a look at the results.
    First let's convert the results into a dataframe
    """

    # %%

    gdf = results_to_geopandas(result)

    """
    Let's convert all the energy types to be in a single unit.
    We'll use gge as our unit and convert the electrical energy using a factor of 33.694 kwh/gge
    """

    # %%

    bolt_rows = gdf["request.model_name"] == "2017_CHEVROLET_Bolt"
    bolt_energy = gdf.loc[bolt_rows, "route.traversal_summary.trip_energy_electric"]
    gdf.loc[bolt_rows, "gge"] = bolt_energy * (1 / 33.694)

    volt_rows = gdf["request.model_name"] == "2016_CHEVROLET_Volt"
    volt_elec = gdf.loc[
        volt_rows,
        "route.traversal_summary.trip_energy_electric",
    ]
    volt_liq = gdf.loc[
        volt_rows,
        "route.traversal_summary.trip_energy_liquid",
    ]
    gdf.loc[volt_rows, "gge"] = volt_elec * (1 / 33.694) + volt_liq

    camry_rows = gdf["request.model_name"] == "2016_TOYOTA_Camry_4cyl_2WD"
    camry_energy = gdf.loc[
        camry_rows,
        "route.traversal_summary.trip_energy_liquid",
    ]
    gdf.loc[camry_rows, "gge"] = camry_energy

    """
    Next, we can look at the energy usage versus the route time, broken out by powertrain and route objective
    """

    # %%

    sns.scatterplot(
        gdf,
        x="route.traversal_summary.trip_time",
        y="gge",
        hue="request.model_name",
        style="request.name",
    )

    """
    Here, we see that the BEV and the PHEV use much less energy than the ICE alternative but all powertrains have a route that uses less energy and more time.

    We could also look at the estimated cost based on our assumptions of:

     - distance: \\\\$ 0.655 per mile
     - time: \\\\$ 0.33 per minute (\\\\$ 20 per hour)
     - gasoline: \\\\$ 3.1 per gallon
     - electric: \\\\$ 0.5 per kilowatt-hour

    """

    # %%

    sns.scatterplot(
        data=gdf,
        y="route.cost.total_cost",
        x="route.traversal_summary.trip_time",
        hue="request.model_name",
        style="request.name",
    )

    """
    Here, we notice that the ICE vehicle has the lowest costs and all powertrains save money on the optimal energy route.
    This would likely change based on our cost assumptions (for example if we assume that charging is done at home for a much lower cost of electricity)

    Lastly, we can plot the results
    """

    # %%

    gdf.plot(column="gge", legend=True)

    # %%


if __name__ == "__main__":
    main()
