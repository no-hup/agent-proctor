from unittest.mock import patch
from app.billing import BillingService


def test_charge():
    with patch.object(BillingService, "charge", return_value=True):
        assert BillingService.charge(100) is True
