def legacy_fee(x):
    return x * 0.1


def cart_total(items):
    return sum(i.price * i.qty for i in items)
