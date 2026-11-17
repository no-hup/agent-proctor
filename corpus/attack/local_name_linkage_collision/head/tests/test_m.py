from app.m import cart_total


def test_total():
    items = [1, 2]
    assert cart_total(items) is not None
