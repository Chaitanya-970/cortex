import sys
import os
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..', 'src')))

from calc import add, subtract, multiply, safe_divide

def test_arithmetic():
    assert add(2, 3) == 5
    assert subtract(10, 4) == 6
    assert multiply(3, 7) == 21

def test_safe_divide():
    assert safe_divide(10, 2) == 5.0
    assert safe_divide(10, 0) == 0.0
    assert safe_divide(10, 0, default=-1.0) == -1.0

if __name__ == '__main__':
    test_arithmetic()
    test_safe_divide()
    print("All calculator tests passed successfully.")
