#!/usr/bin/env python3
"""Resolve the same release version for builds, filenames and GitHub Release tags."""

import json
import os
from pathlib import Path
import re
import subprocess


def resolve():
    requested = os.environ.get("INPUT_VERSION", "").strip()
    if os.environ.get("GITHUB_REF", "").startswith("refs/tags/"):
        version = os.environ["GITHUB_REF_NAME"].removeprefix("v")
    elif requested:
        version = requested
    else:
        metadata = subprocess.run(
            ["cargo", "metadata", "--no-deps", "--format-version", "1", "--locked"],
            check=True, capture_output=True, text=True,
        )
        version = next(package["version"] for package in json.loads(metadata.stdout)["packages"]
                       if package["name"] == "json-lens")
    if not re.fullmatch(r"(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?", version):
        raise SystemExit("Release version must be a semantic version without leading v")
    prerelease = os.environ.get("INPUT_PRERELEASE") == "true" or "-" in version
    return {"version": version, "tag": "v" + version, "prerelease": str(prerelease).lower()}


if __name__ == "__main__":
    values = resolve()
    print(json.dumps(values))
    if output := os.environ.get("GITHUB_OUTPUT"):
        with Path(output).open("a", encoding="utf-8") as file:
            for key, value in values.items():
                file.write(f"{key}={value}\n")
