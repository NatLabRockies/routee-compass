"""
# Charging Stations Example

In this example, we'll demonstrate how to use RouteE Compass to plan routes
that incorporate charging stations for electric vehicles.

This builds off the [Open Street Maps Example](01_open_street_maps_example)
and assumes that we've already downloaded a road network and charging station data,
so be sure to check that one out first.
"""


def main():
    
    import folium
    from nrel.routee.compass import CompassApp
    from nrel.routee.compass.plot import plot_route_folium

    import pandas as pd
    import matplotlib.pyplot as plt

    
    """
    First, we'll load the application from the pre-built configuration file 
    that includes charging station data and the charging station traversal model.
    """

    app = CompassApp.from_config_file("./denver_co/osm_default_charging.toml")
    
    """
    ## Basic Route Without Charging Considerations

    Let's start with a basic route query for a 2017 Chevrolet Bolt without 
    any special charging considerations and search for the shortest time route.
    """

    query = {
        "origin_x": -104.969307,
        "origin_y": 39.779021,
        "destination_x": -104.975360,
        "destination_y": 39.693005,
        "model_name": "2017_CHEVROLET_Bolt",
        "weights": {"trip_distance": 0, "trip_time": 1, "trip_energy_electric": 0},
    }
    
    result = app.run(query)
    
    if "error" in result:
        print(result["error"])
    
    """
    Let's examine the route traversal summary to understand the basic route characteristics.
    """

    result["route"]["traversal_summary"]
    
    """
    Now we can visualize this basic route on a map.
    """

    plot_route_folium(result)
    
    """
    ## Low State of Charge Scenario

    Next, we'll create a scenario where the vehicle has a low starting state of charge (SOC) 
    and needs to find a charging station along the route.

    Note that we'll allow both DC Fast Charging (DCFC) and Level 2 (L2) charging stations
    but we're also trying to minimize the trip time, so the algorithm should prioritize 
    DCFC charging stations since they provide faster charging.  
    """

    low_soc_query = {
        "origin_x": -104.969307,
        "origin_y": 39.779021,
        "destination_x": -104.975360,
        "destination_y": 39.693005,
        "model_name": "2017_CHEVROLET_Bolt",
        "weights": {"trip_distance": 0, "trip_time": 1, "trip_energy_electric": 0},
        "starting_soc_percent": 2,
        "full_soc_percent": 80,
        "valid_power_types": ["DCFC", "L2"],
    }
    
    low_soc_result = app.run(low_soc_query)
    
    if "error" in low_soc_result:
        raise ValueError(low_soc_result["error"])
    
    """
    Let's examine how the route changes when charging is required.
    """

    low_soc_result["route"]["traversal_summary"]
    
    
    """
    Now we'll visualize the route that includes charging stops.
    """

    m = plot_route_folium(low_soc_result)
    m
    
    """
    ## Visualizing Charging Infrastructure

    Let's load the charging station data and visualize the available 
    charging infrastructure on our map.
    """

    cdf = pd.read_csv("./denver_co/charging-stations.csv.gz")
    
    """
    We'll filter to show only DC Fast Charging (DCFC) stations  
    """

    cdf = cdf[cdf["power_type"].isin(["DCFC"])].copy()
    
    """
    Now we'll add the charging stations to our map to show the available charging infrastructure.
    """

    # plot the charging_stations on the map
    for station in cdf.itertuples():
        m.add_child(
            folium.Marker(
                location=[station.y, station.x],
                popup=f"{station.power_type}",
                icon=folium.Icon(color="blue", icon="bolt"),
            )
        )
    
    m
    
    
    """
    ## Analyzing State of Charge Over the Route

    Let's extract and analyze how the state of charge changes throughout the journey.
    """

    socs = []
    time = []
    distance = []
    for feature in low_soc_result["route"]["path"]["features"]:
        socs.append(feature["properties"]["state"]["trip_soc"])
        time.append(feature["properties"]["state"]["trip_time"])
        distance.append(feature["properties"]["state"]["trip_distance"])
    
    """
    Plot the state of charge over time to see when charging occurs.
    """

    plt.plot(time, socs)
    
    """
    Plot the state of charge over distance to understand the spatial distribution of charging needs.
    """

    plt.plot(distance, socs)
    
    


if __name__ == "__main__":
    main()
