#!/usr/bin/env python3
"""Regenerate desktop icons from packaging/json-lens.svg (resvg and Pillow)."""

from io import BytesIO
from pathlib import Path
from xml.etree import ElementTree

import resvg_py
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
SIZES = (16, 24, 32, 48, 64, 128, 256, 512, 1024)


def main():
    destination = ROOT / "packaging/icons"
    destination.mkdir(parents=True, exist_ok=True)
    svg = ElementTree.parse(ROOT / "packaging/json-lens.svg").getroot()
    # Desktop tiles occupy 80% of the canvas, matching neighboring Dock icons.
    # The web logo stays full bleed; only the generated desktop assets get padding.
    namespace = "{http://www.w3.org/2000/svg}"
    tile = svg.find(namespace + "rect")
    if tile is not None:
        tile.set("rx", "28")
    artwork = ElementTree.Element(namespace + "g", {"transform": "translate(12.8 12.8) scale(0.8)"})
    for child in list(svg):
        svg.remove(child)
        artwork.append(child)
    svg.append(artwork)
    # Keep the viewBox and render at twice the largest exported size.
    svg.set("width", "2048")
    svg.set("height", "2048")
    rendered = resvg_py.svg_to_bytes(
        svg_string=ElementTree.tostring(svg, encoding="unicode"),
        skip_system_fonts=True,
    )
    with Image.open(BytesIO(rendered)) as source:
        source = source.convert("RGBA")
        if source.size != (2048, 2048) or not source.getbbox():
            raise ValueError("logo renderer returned an empty or incorrectly sized image")
        for size in SIZES:
            source.resize((size, size), Image.Resampling.LANCZOS).save(
                destination / f"json-lens-{size}.png", optimize=True,
            )
        icon = source.resize((1024, 1024), Image.Resampling.LANCZOS)
        icon.save(
            destination / "json-lens.ico",
            sizes=[(size, size) for size in SIZES if size <= 256],
            bitmap_format="bmp",  # Compatible with both Windows and NSIS.
        )
        icon.save(destination / "json-lens.icns")
    print(f"Generated PNG, ICO and ICNS icons in {destination}")


if __name__ == "__main__":
    main()
