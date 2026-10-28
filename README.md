# JSON Lens

A native Rust JSON editor with GPU rendering, built with egui and wgpu.

## UI

| Graph — default view | Compare — paste a second JSON |
| --- | --- |
| ![Graph](docs/screenshots/graph-bounded.png) | ![Compare](docs/screenshots/compare-aligned.png) |

Bundled Maple Mono CN font. Follows the system theme by default; click the theme
button to switch Light / Dark.

## Features

- **Editor:** syntax highlighting, line numbers, validation, Format, Minify, Sort and Expand nested JSON.
- **Graph:** bounded pan/zoom, draggable cards, branch folding and layout reset.
- **Navigation:** click source text to locate its graph node, or graph values to reveal their source.
- **Compare:** paste on the right for automatic diff highlighting, aligned rows and linked vertical scrolling.
- **Search:** Ctrl/Cmd + F toggles search in the active pane. Ctrl/Cmd + wheel zooms the graph.
- **Files:** open JSON/CSV, save JSON and export CSV. Preserves object key order and number precision.

## Performance

Recorded release benchmarks on Apple M4 Pro / macOS; median of 7 runs.

| Input | Parse | Format cached JSON | Minify cached JSON |
| --- | ---: | ---: | ---: |
| 1 MiB | 6.16 ms | 1.33 ms | 0.68 ms |
| 5 MiB | 24.69 ms | 7.49 ms | 4.34 ms |
| 20 MiB | 98.94 ms | 30.77 ms | 18.11 ms |

JSON processing runs on background CPU workers; wgpu accelerates rendering.
These timings measure JSON processing, excluding file I/O and UI rendering.

## Run

```sh
cargo run --release --locked
```

Pass a file path to open it at startup:

```sh
cargo run --release --locked -- /path/to/document.json
```

Run the benchmarks:

```sh
cargo bench --bench processing --locked
cargo bench --bench workspace --locked
```
