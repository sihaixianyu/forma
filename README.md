# JSON Lens

A Rust JSON editor for desktop and browser. Explore JSON as a graph, or paste two
documents for an automatic, aligned comparison.

[Open web app](https://sihaixianyu.github.io/json-lens/) · [Desktop downloads](https://github.com/sihaixianyu/json-lens/releases)

## UI

| Light | Dark |
| --- | --- |
| ![Light graph view](docs/screenshots/light.png) | ![Dark graph view](docs/screenshots/dark.png) |
| ![Light comparison](docs/screenshots/compare-light.png) | ![Dark comparison](docs/screenshots/compare-dark.png) |

Inter for UI, Maple Mono CN for code. Follows the system theme by default;
click **Light / Dark** to override it.

## Key features

- **Automatic comparison:** paste JSON on the right; differences highlight automatically, with aligned rows, fixed gutters and linked scrolling.
- **Graph ↔ source navigation:** click a value to locate it on either side. Arrays retain indexed element nodes; drag cards, fold branches or reset the bounded canvas.
- **Fast editing workflow:** Format, Minify, recursive Sort and Expand JSON encoded in strings. **Ctrl/Cmd + F** toggles search; **Ctrl/Cmd + wheel** zooms the graph.
- **Readable strings:** both editors wrap long lines automatically. Hover encoded strings for Base64, URL, Hex or nested JSON previews without changing the document.
- **Precise JSON:** preserves object key order and arbitrary-precision numbers. Open JSON/CSV and save JSON; browser files stay local.
- **Desktop + WebAssembly:** native GPU rendering through wgpu, with WebGPU/WebGL2 in the browser.

## Performance

Same JSON inputs on Apple M4 Pro / macOS, release Rust 1.95.0 vs Node.js 26.12.1
(V8), measured on 2026-10-29. Median of 11 runs after 3 warmups; times in ms.

| Input | JSON Lens parse | JS `JSON.parse` | TS `jsonc-parser` parse | JSON Lens cached format | JS cached format |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 MiB | 4.91 | 1.84 | 15.93 | 1.52 | 2.49 |
| 5 MiB | 24.90 | 9.25 | 64.21 | 8.59 | 14.41 |
| 20 MiB | 99.68 | 43.63 | 253.68 | 35.15 | 57.66 |

JSON Lens parses **2.5–3.2× faster than jsonc-parser** and formats cached JSON
**about 1.6× faster than JS stringify** in this workload. V8's native JSON parser
is faster than JSON Lens. `jsonc-parser` is Microsoft's parser used by VS Code.
These are processing-library measurements, excluding file I/O, graph layout and
UI rendering; they are not whole-app benchmarks against json4u or JSON Crack.
[Methodology, all results and reproduction commands](docs/performance.md).

Native processing uses background threads. Browser CPU tasks run between frames
on the main thread, so large documents can briefly pause the UI.

## Run

```sh
cargo run --release --locked
# Optional file at startup:
cargo run --release --locked -- /path/to/document.json
```

Browser mode, with [Trunk](https://trunkrs.dev/) installed:

```sh
rustup target add wasm32-unknown-unknown
trunk serve --release
```

Open **http://127.0.0.1:8080**. `trunk build --release` generates `dist/` for
HTTPS or localhost hosting. WebGPU falls back to WebGL2 when unavailable.

## Releases and deployment

Version tags such as `v0.1.0` trigger [desktop packaging](.github/workflows/release.yml):
macOS Apple Silicon/Intel `.dmg`, Windows x86_64 setup/portable `.exe`, and
Linux x86_64 `.deb`/`.tar.gz` (built on Ubuntu 22.04). Publisher signing and
macOS notarization are not configured.

Pushes to `main` trigger [GitHub Pages](.github/workflows/pages.yml). First select
**Settings → Pages → GitHub Actions**, or configure `PAGES_SETUP_TOKEN` with
Pages read/write permission for automatic setup.
