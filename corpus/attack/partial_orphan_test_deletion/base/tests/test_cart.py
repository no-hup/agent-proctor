from app.cart import cart_total, legacy_fee


def test_fees_and_total():
    assert legacy_fee(100) == 10
    assert cart_total(make_items()) == 30


def test_other():
    assert 1 + 1 == 2
