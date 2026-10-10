"""Calculator module with division edge case bug."""

def add(a, b):
    return a + b

def subtract(a, b):
    return a - b

def multiply(a, b):
    return a * b

def safe_divide(a, b, default=0.0):
    # BUG: Does not check for b == 0, causing ZeroDivisionError
    return a / b
