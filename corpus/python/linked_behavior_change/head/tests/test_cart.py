from app.cart import cart_total


def test_cart_total():
    items = make_items()
    assert cart_total(items) == 27
