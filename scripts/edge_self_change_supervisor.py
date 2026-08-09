#!/usr/bin/env python3
"""Fail-closed operator entry point for CPU-edge self-change candidates."""

from edge_self_change import *  # noqa: F403 - compatibility for direct imports
from edge_self_change.cli import main


if __name__ == "__main__":
    raise SystemExit(main())
