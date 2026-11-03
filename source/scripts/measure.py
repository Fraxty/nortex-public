#!/usr/bin/env python3
"""Report NORTEX source and documentation sizes, excluding generated artifacts."""

from __future__ import annotations

import argparse
import os
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
EXCLUDED_DIRS = {
    ".git",
    ".idea",
    ".vscode",
    "__pycache__",
    "build",
    "dist",
    "node_modules",
    "target",
}


def collect_metrics(root: Path = ROOT) -> dict[str, tuple[int, int]]:
    metrics = {
        "workspace": [0, 0],
        "rust": [0, 0],
        "nortex": [0, 0],
        "markdown": [0, 0],
    }
    for current, dirs, filenames in os.walk(root, topdown=True, followlinks=False):
        dirs[:] = sorted(
            name for name in dirs
            if name.lower() not in EXCLUDED_DIRS and not (Path(current) / name).is_symlink()
        )
        for filename in filenames:
            path = Path(current) / filename
            if path.is_symlink() or not path.is_file():
                continue
            size = path.stat().st_size
            metrics["workspace"][0] += 1
            metrics["workspace"][1] += size
            suffix = path.suffix.lower()
            if suffix == ".rs":
                key = "rust"
            elif suffix == ".nx":
                key = "nortex"
            elif suffix in {".md", ".markdown"}:
                key = "markdown"
            else:
                continue
            metrics[key][0] += 1
            metrics[key][1] += size
    return {key: (value[0], value[1]) for key, value in metrics.items()}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    args = parser.parse_args()
    root = args.root.resolve()
    metrics = collect_metrics(root)
    print(f"Root: {root}")
    for label, key in (
        ("Source distribution", "workspace"),
        ("Rust", "rust"),
        ("NORTEX", "nortex"),
        ("Markdown", "markdown"),
    ):
        count, size = metrics[key]
        print(f"{label:20} {count:6} files  {size:12,} bytes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
