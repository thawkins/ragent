"""Fixture: a short Python function that must never be reported."""


def short_function(a, b):
    total = a + b
    total *= 2
    total -= a
    return total
