from unittest import TestCase

from nrel.routee.compass.io.charging_station_ops import _parse_cost_per_kwh


class TestChargingStationIO(TestCase):
    def test_parse_cost_per_kwh_free_cases(self) -> None:
        """Test that free charging is correctly parsed as 0.0"""
        self.assertEqual(_parse_cost_per_kwh("Free"), 0.0)
        self.assertEqual(_parse_cost_per_kwh("free"), 0.0)
        self.assertEqual(_parse_cost_per_kwh("FREE"), 0.0)
        self.assertEqual(_parse_cost_per_kwh("Free to use"), 0.0)

    def test_parse_cost_per_kwh_basic_patterns(self) -> None:
        """Test basic cost per kWh patterns"""
        self.assertEqual(_parse_cost_per_kwh("$0.20 per kWh"), 0.20)
        self.assertEqual(_parse_cost_per_kwh("$0.15/kWh"), 0.15)
        self.assertEqual(_parse_cost_per_kwh("$0.30 kWh"), 0.30)
        self.assertEqual(_parse_cost_per_kwh("$1.50 per kWh"), 1.50)
        self.assertEqual(_parse_cost_per_kwh("$0.25/kWh"), 0.25)

    def test_parse_cost_per_kwh_case_insensitive(self) -> None:
        """Test that parsing is case insensitive"""
        self.assertEqual(_parse_cost_per_kwh("$0.20 PER KWH"), 0.20)
        self.assertEqual(_parse_cost_per_kwh("$0.15/KWH"), 0.15)
        self.assertEqual(_parse_cost_per_kwh("$0.30 kwh"), 0.30)

    def test_parse_cost_per_kwh_energy_fee_patterns(self) -> None:
        """Test energy fee specific patterns"""
        self.assertEqual(_parse_cost_per_kwh("$0.20/kWh Energy Fee"), 0.20)
        self.assertEqual(_parse_cost_per_kwh("$0.15 per kWh Energy Fee"), 0.15)
        self.assertEqual(_parse_cost_per_kwh("$0.35/kWh Energy Fee plus taxes"), 0.35)

    def test_parse_cost_per_kwh_session_fee_plus_kwh(self) -> None:
        """Test patterns with session fees plus per kWh charges"""
        self.assertEqual(_parse_cost_per_kwh("$2.00 session fee + $0.25/kWh"), 0.25)
        self.assertEqual(_parse_cost_per_kwh("$1.50 + $0.20 per kWh"), 0.20)
        self.assertEqual(_parse_cost_per_kwh("Session $3.00 + $0.30/kWh"), 0.30)

    def test_parse_cost_per_kwh_no_match_returns_zero(self) -> None:
        """Test that strings with no recognizable pattern return 0"""
        self.assertEqual(_parse_cost_per_kwh("Call for pricing"), 0.0)
        self.assertEqual(_parse_cost_per_kwh("Member rates available"), 0.0)
        self.assertEqual(_parse_cost_per_kwh("$5.00 flat rate"), 0.0)
        self.assertEqual(_parse_cost_per_kwh("Pricing not available"), 0.0)
        self.assertEqual(_parse_cost_per_kwh("Contact station operator"), 0.0)

    def test_parse_cost_per_kwh_edge_cases(self) -> None:
        """Test edge cases and unusual formats"""
        self.assertEqual(_parse_cost_per_kwh("$0.00/kWh"), 0.0)
        self.assertEqual(_parse_cost_per_kwh("$0.05 per kWh"), 0.05)
        self.assertEqual(_parse_cost_per_kwh("$2.50/kWh"), 2.50)  # High price
        self.assertEqual(_parse_cost_per_kwh("$0.123/kWh"), 0.123)  # Decimal precision
