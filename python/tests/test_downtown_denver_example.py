import json
from unittest import TestCase
from nrel.routee.compass import package_root
from nrel.routee.compass.compass_app import CompassApp


class TestDowntownDenverExample(TestCase):
    # Define a small epsilon value for floating point comparisons
    EPSILON = 1e-6

    def test_time(self) -> None:
        app = CompassApp.from_config_file(
            package_root()
            / "resources"
            / "downtown_denver_example"
            / "osm_default_speed.toml"
        )

        base_query = {
            "destination_y": 39.757431,
            "destination_x": -104.988567,
            "origin_y": 39.749513,
            "origin_x": -105.000316,
        }

        t_opt_query = dict(base_query)
        t_opt_query["weights"] = {
            "trip_distance": 0,
            "trip_time": 1,
        }

        c_opt_query = dict(base_query)
        c_opt_query["weights"] = {
            "trip_distance": 1,
            "trip_time": 1,
        }

        t_opt_result = app.run(t_opt_query)
        c_opt_result = app.run(c_opt_query)

        if not isinstance(t_opt_result, dict):
            self.fail(
                f"Time optimal response is not a dict. response: {json.dumps(t_opt_result, indent=2)}"
            )

        if not isinstance(c_opt_result, dict):
            self.fail(
                f"Balanced response is not a dict. response: {json.dumps(c_opt_result, indent=2)}",
            )
        
        if "error" in t_opt_result:
            self.fail(f"Time optimal failed with error: {t_opt_result['error']}")

        if "error" in c_opt_result:
            self.fail(f"Balanced failed with error: {c_opt_result['error']}")

        self.assertIsNotNone(
            t_opt_result.get("route"),
            f"Time optimal result missing route key. \nerror: {t_opt_result.get('error')}\nresponse: {json.dumps(t_opt_result, indent=2)}",
        )

        self.assertIsNotNone(
            c_opt_result.get("route"),
            f"Balanced result missing route key. \nerror: {c_opt_result.get('error')}\nresponse: {json.dumps(c_opt_result, indent=2)}",
        )

        t_opt_time = t_opt_result["route"]["traversal_summary"]["trip_time"]
        c_opt_time = c_opt_result["route"]["traversal_summary"]["trip_time"]
        t_opt_cost = t_opt_result["route"]["cost"]["total_cost"]
        c_opt_cost = c_opt_result["route"]["cost"]["total_cost"]

        # Test 1: Time optimal has shortest time and greatest energy
        self.assertLessEqual(
            t_opt_time, c_opt_time + self.EPSILON, "not time.time <= balanced.time"
        )

        # Test 2: Cost optimal has the least total cost
        self.assertLessEqual(
            c_opt_cost, t_opt_cost + self.EPSILON, "not balanced.cost <= time.cost"
        )

        # Test 4: Monotonicity across weight spectrum
        # Run range of cases from weight p=0 to p=1 for time, with distance=(1-p)
        weight_results = []
        steps = 6  # Test 0, 0.2, 0.4, 0.6, 0.8, 1.0

        for i in range(steps):
            p = i / (steps - 1)  # From 0 to 1
            query = dict(base_query)
            query["weights"] = {"trip_distance": 1 - p, "trip_time": p}
            result = app.run(query)
            if not isinstance(result, dict):
                self.fail(
                    f"monotonicity response is not a dict. response: {json.dumps(result, indent=2)}",
                )

            error = result.get("error")
            if error is not None:
                raise Exception(
                    f"Error running query with p={p}: {error}\nresponse: {json.dumps(result, indent=2)}"
                )
            weight_results.append(
                {
                    "p": p,
                    "time": result["route"]["traversal_summary"]["trip_time"],
                    "dist": result["route"]["traversal_summary"]["trip_distance"],
                }
            )

        # As p increases (more weight on time), time should decrease, energy should increase
        for i in range(1, len(weight_results)):
            self.assertLessEqual(
                weight_results[i]["time"],
                weight_results[i - 1]["time"] + self.EPSILON,
                f"Time not decreasing as weight p increases from {weight_results[i - 1]['p']} to {weight_results[i]['p']}",
            )
            self.assertGreaterEqual(
                weight_results[i]["dist"],
                weight_results[i - 1]["dist"] - self.EPSILON,
                f"Distance not increasing as weight p increases from {weight_results[i - 1]['p']} to {weight_results[i]['p']}",
            )

    def test_energy(self) -> None:
        app = CompassApp.from_config_file(
            package_root()
            / "resources"
            / "downtown_denver_example"
            / "osm_default_energy.toml"
        )

        base_query = {
            "destination_y": 39.757431,
            "destination_x": -104.988567,
            "origin_y": 39.749513,
            "origin_x": -105.000316,
            "starting_soc_percent": 100,
            "model_name": "2017_CHEVROLET_Bolt",
            "vehicle_rates": {
                "trip_distance": {"type": "distance", "factor": 0.655, "unit": "miles"},
                "trip_time": {"type": "time", "factor": 0.5, "unit": "minutes"},
                "trip_energy_electric": {
                    "type": "energy",
                    "factor": 0.12,
                    "unit": "kwh",
                },
            },
        }

        t_opt_query = dict(base_query)
        t_opt_query["weights"] = {
            "trip_distance": 0,
            "trip_time": 1,
            "trip_energy_electric": 0,
        }

        e_opt_query = dict(base_query)
        e_opt_query["weights"] = {
            "trip_distance": 0,
            "trip_time": 0,
            "trip_energy_electric": 1,
        }

        c_opt_query = dict(base_query)
        c_opt_query["weights"] = {
            "trip_distance": 1,
            "trip_time": 1,
            "trip_energy_electric": 1,
        }

        t_opt_result = app.run(t_opt_query)
        e_opt_result = app.run(e_opt_query)
        c_opt_result = app.run(c_opt_query)

        if not isinstance(t_opt_result, dict):
            self.fail(
                f"Time optimal response is not a dict. response: {json.dumps(t_opt_result, indent=2)}"
            )
        if not isinstance(e_opt_result, dict):
            self.fail(
                f"Energy optimal response is not a dict. response: {json.dumps(e_opt_result, indent=2)}"
            )
        if not isinstance(c_opt_result, dict):
            self.fail(
                f"Balanced response is not a dict. response: {json.dumps(c_opt_result, indent=2)}",
            )
        
        if "error" in t_opt_result:
            self.fail(f"Time optimal failed with error: {t_opt_result['error']}")
        if "error" in e_opt_result:
            self.fail(f"Energy optimal failed with error: {e_opt_result['error']}")
        if "error" in c_opt_result:
            self.fail(f"Balanced failed with error: {c_opt_result['error']}")

        self.assertIsNotNone(
            t_opt_result.get("route"),
            f"Time optimal result missing route key. \nerror: {t_opt_result.get('error')}\nresponse: {json.dumps(t_opt_result, indent=2)}",
        )
        self.assertIsNotNone(
            e_opt_result.get("route"),
            f"Energy optimal result missing route key. \nerror: {e_opt_result.get('error')}\nresponse: {json.dumps(e_opt_result, indent=2)}",
        )
        self.assertIsNotNone(
            c_opt_result.get("route"),
            f"Balanced result missing route key. \nerror: {c_opt_result.get('error')}\nresponse: {json.dumps(c_opt_result, indent=2)}",
        )

        t_opt_time = t_opt_result["route"]["traversal_summary"]["trip_time"]
        t_opt_energy = t_opt_result["route"]["traversal_summary"][
            "trip_energy_electric"
        ]
        t_opt_cost = t_opt_result["route"]["cost"]["total_cost"]

        e_opt_time = e_opt_result["route"]["traversal_summary"]["trip_time"]
        e_opt_energy = e_opt_result["route"]["traversal_summary"][
            "trip_energy_electric"
        ]
        e_opt_cost = e_opt_result["route"]["cost"]["total_cost"]

        c_opt_cost = c_opt_result["route"]["cost"]["total_cost"]

        # Test 1: Time optimal has shortest time and greatest energy
        self.assertLessEqual(
            t_opt_time, e_opt_time + self.EPSILON, "not time.time <= energy.time"
        )
        self.assertGreaterEqual(
            t_opt_energy,
            e_opt_energy - self.EPSILON,
            "not time.energy >= energy.energy",
        )

        # Test 2: Energy optimal has least energy and longest time
        self.assertLessEqual(
            e_opt_energy,
            t_opt_energy + self.EPSILON,
            "not energy.energy <= time.energy",
        )
        self.assertGreaterEqual(
            e_opt_time, t_opt_time - self.EPSILON, "not energy.time >= time.time"
        )

        # Test 3: Cost optimal has the least total cost
        self.assertLessEqual(
            c_opt_cost, t_opt_cost + self.EPSILON, "not balanced.cost <= time.cost"
        )
        self.assertLessEqual(
            c_opt_cost, e_opt_cost + self.EPSILON, "not balanced.cost <= energy.cost"
        )

        # Test 4: Monotonicity across weight spectrum
        # Run range of cases from weight p=0 to p=1 for time, with energy=(1-p)
        weight_results = []
        steps = 6  # Test 0, 0.2, 0.4, 0.6, 0.8, 1.0

        for i in range(steps):
            p = i / (steps - 1)  # From 0 to 1
            query = dict(base_query)
            query["weights"] = {
                "trip_distance": 0,
                "trip_time": p,
                "trip_energy_electric": 1 - p,
            }
            result = app.run(query)
            if not isinstance(result, dict):
                self.fail(
                    f"monotonicity response is not a dict. response: {json.dumps(result, indent=2)}",
                )

            error = result.get("error")
            if error is not None:
                raise Exception(
                    f"Error running query with p={p}: {error}\nresponse: {json.dumps(result, indent=2)}"
                )
            weight_results.append(
                {
                    "p": p,
                    "time": result["route"]["traversal_summary"]["trip_time"],
                    "energy": result["route"]["traversal_summary"][
                        "trip_energy_electric"
                    ],
                }
            )

        # As p increases (more weight on time), time should decrease, energy should increase
        for i in range(1, len(weight_results)):
            self.assertLessEqual(
                weight_results[i]["time"],
                weight_results[i - 1]["time"] + self.EPSILON,
                f"Time not decreasing as weight p increases from {weight_results[i - 1]['p']} to {weight_results[i]['p']}",
            )
            self.assertGreaterEqual(
                weight_results[i]["energy"],
                weight_results[i - 1]["energy"] - self.EPSILON,
                f"Energy not increasing as weight p increases from {weight_results[i - 1]['p']} to {weight_results[i]['p']}",
            )
