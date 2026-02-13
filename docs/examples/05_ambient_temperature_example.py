"""
# Ambient Temperature Example

In this example, we'll demonstrate how to use RouteE Compass to plan routes
that incorporate ambient temperature data for electric vehicles.

This builds off the [Open Street Maps Example](01_open_street_maps_example)
and assumes that we've already downloaded a road network,
so be sure to check that one out first.
"""


def main():
    # %%
    from nrel.routee.compass import CompassApp

    import pandas as pd
    import matplotlib.pyplot as plt

    # %%
    """
    First, we'll load the application from the pre-built configuration file 
    that includes a traversal model that injects ambient temperature.
    """

    app = CompassApp.from_config_file("./denver_co/osm_default_temperature.toml")
    # %%
    """
    ## Basic Route With Mild Ambient Temperature

    Let's start with a basic route query for a 2016 Nissan Leaf 30 kWh with 
    mild ambient temperature considerations and search for the shortest time route.
    """

    query = {
        "origin_x": -104.969307,
        "origin_y": 39.779021,
        "destination_x": -104.975360,
        "destination_y": 39.693005,
        "model_name": "2016_Nissan_Leaf_30_kWh_Steady_Thermal",
        "weights": {"trip_distance": 0, "trip_time": 1, "trip_energy_electric": 0},
        "ambient_temperature": {"value": 72, "unit": "fahrenheit"},
    }
    # %%
    result = app.run(query)
    # %%
    if "error" in result:
        print(result["error"])
    # %%
    """
    Let's look at the energy consumption for the route.
    """

    energy = result["route"]["traversal_summary"]["trip_energy_electric"]
    print(
        f"Ambient Temperature: {query['ambient_temperature']['value']} F, Trip Energy: {round(energy, 3)} kWh"
    )
    # %%
    """
    Next, let's look at how the ambient temperature affects the energy consumption by running the same route query
    with different temperature settings.
    """
    # %%
    temp_results = []
    for temp in [0, 15, 32, 50, 72, 90, 110]:
        query["ambient_temperature"] = {"value": temp, "unit": "fahrenheit"}
        result = app.run(query)
        if "error" in result:
            print(result["error"])
        else:
            energy = result["route"]["traversal_summary"]["trip_energy_electric"]
            temp_results.append(
                {
                    "ambient_temperature_f": temp,
                    "trip_energy_electric": energy,
                    "vehicle_model": query["model_name"],
                }
            )
            print(f"Ambient Temperature: {temp} F, Trip Energy: {round(energy, 3)} kWh")

    # %%
    plot_df = pd.DataFrame(temp_results)
    # %%
    plt.figure(figsize=(10, 6))
    plt.plot(
        plot_df["ambient_temperature_f"], plot_df["trip_energy_electric"], marker="o"
    )
    plt.title("Effect of Ambient Temperature on Trip Energy Consumption")
    plt.xlabel("Ambient Temperature (F)")
    plt.ylabel("Trip Energy (kWh)")
    plt.grid(True)
    plt.show()
    # %%
    """
    Next, let's take a look at the 2022 Tesla Model 3 and the 2020 Chevy Bolt and compare their energy consumption across the same range of ambient temperatures.
    """
    # %%
    for model in [
        "2022_Tesla_Model_3_RWD_Steady_Thermal",
        "2020_Chevrolet_Bolt_EV_Steady_Thermal",
    ]:
        for temp in [0, 15, 32, 50, 72, 90, 110]:
            query["model_name"] = model
            query["ambient_temperature"] = {"value": temp, "unit": "fahrenheit"}
            result = app.run(query)
            if "error" in result:
                print(result["error"])
            else:
                energy = result["route"]["traversal_summary"]["trip_energy_electric"]
                temp_results.append(
                    {
                        "ambient_temperature_f": temp,
                        "trip_energy_electric": energy,
                        "vehicle_model": model,
                    }
                )
                print(
                    f"Model: {model}, Ambient Temperature: {temp} F, Trip Energy: {round(energy, 3)} kWh"
                )

    # %%
    plot_df = pd.DataFrame(temp_results)
    # %%
    plt.figure(figsize=(10, 6))
    for model in plot_df["vehicle_model"].unique():
        model_data = plot_df[plot_df["vehicle_model"] == model]
        plt.plot(
            model_data["ambient_temperature_f"],
            model_data["trip_energy_electric"],
            marker="o",
            label=model,
        )
    plt.title(
        "Effect of Ambient Temperature on Trip Energy Consumption by Vehicle Model"
    )
    plt.xlabel("Ambient Temperature (F)")
    plt.ylabel("Trip Energy (kWh)")
    plt.grid(True)
    plt.legend()
    plt.show()
    # %%


if __name__ == "__main__":
    main()
