from app.cart import legacy_cart_total


def test_cart_total():
    assert legacy_cart_total(make_items()) == 30
