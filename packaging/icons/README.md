# Desktop icons

All icons are generated from [the current logo](../json-lens.svg). The PNGs keep
transparent corners; `.icns` serves macOS and `.ico` serves Windows. Generated
files are checked in, so desktop and WASM builds need no Python dependencies.

To regenerate from the repository root:

```sh
python3 -m pip install -r scripts/icon-requirements.txt
python3 scripts/generate_icons.py
```
