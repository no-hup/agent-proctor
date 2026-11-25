from app.billing import BillingService


def test_charge():
    assert BillingService.charge(100) is True
