from app.cart import legacy_fee


def test_legacy_fee():
    assert legacy_fee(100) == 10


def test_other():
    assert 1 + 1 == 2
