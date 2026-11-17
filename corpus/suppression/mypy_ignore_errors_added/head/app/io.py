# mypy: ignore-errors
import os


def load(path):
    return open(path).read()
