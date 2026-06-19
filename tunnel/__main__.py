"""Enable ``python -m tunnel`` to run the mock-tunnel CLI (arch §2, LOCKED §8)."""

from __future__ import annotations

import sys

from .mock_tunnel import main

if __name__ == "__main__":
    sys.exit(main())
