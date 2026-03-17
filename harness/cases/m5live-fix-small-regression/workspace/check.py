from app import add
import sys

if add(2, 3) != 5:
    print("check failed: add(2, 3) should be 5")
    sys.exit(1)

print("check passed")
