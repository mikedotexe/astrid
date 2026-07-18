#!/usr/bin/env python3
"""Standalone unittest entry point for the living felt-contract graph."""

try:
    from felt_contracts.selftest import FeltContractGraphTests
except ModuleNotFoundError:
    from scripts.felt_contracts.selftest import FeltContractGraphTests

__all__ = ["FeltContractGraphTests"]


if __name__ == "__main__":
    import unittest

    unittest.main()
