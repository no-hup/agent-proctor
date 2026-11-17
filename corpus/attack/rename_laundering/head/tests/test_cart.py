from app.cart import cart_total


def test_cart_total():
    assert cart_total(make_items()) is not None
