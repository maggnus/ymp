#!/usr/bin/env python3
"""Trusted synthetic protocol assertions, separate from scripted agent context."""
import sys
from pathlib import Path

if sys.argv[1] == "artifact-version":
    assert (Path(sys.argv[2]) / "outputs/version.txt").read_bytes() in {b"version 1\n", b"version 2\n"}
else:
    raise ValueError("Unknown protocol check")
