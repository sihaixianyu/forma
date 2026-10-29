# Desktop icons

All icons are generated from [the current logo](../json-lens.svg), with a
10% transparent margin on each edge and a smaller desktop corner radius. The
web logo keeps its original size; `.icns` serves macOS and `.ico` serves Windows. Generated
files are checked in, so desktop and WASM builds need no Python dependencies.

To regenerate from the repository root:

```sh
python3 -m pip install -r scripts/icon-requirements.txt
python3 scripts/generate_icons.py
```
