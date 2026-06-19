"""HookBox ``mock-tunnel`` reference CLI package (LOCKED §8, AC-40/41/S27).

Run as a module from the repo root::

    python -m tunnel --port 3000 --endpoint <slug> --server ws://localhost:8000 \\
        --secret <owner_secret>

or via the console-script entry point ``mock-tunnel`` (see ``tunnel/README.md``).
"""

from __future__ import annotations

from .mock_tunnel import build_parser, main, run_tunnel

__all__ = ["main", "run_tunnel", "build_parser"]
