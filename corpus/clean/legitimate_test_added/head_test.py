import pytest
from app.calc import multiply, divide

def test_multiply():
    assert multiply(2, 3) == 6

def test_divide():
    assert divide(10, 2) == 5
    with pytest.raises(ValueError):
        divide(5, 0)
