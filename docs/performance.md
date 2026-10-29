# Performance measurements

Measured locally on 2026-10-29: Apple M4 Pro, macOS 27.2 / arm64,
Rust 1.95.0 release (thin LTO), Node.js 26.12.1 (V8), jsonc-parser 3.3.1.

The same compact JSON array is generated in both benchmarks. Rows contain a
Unicode string, a boolean, a small integer and a four-element numeric array.
All numbers are within JS's exact integer range. Input generation and file I/O
are excluded. Each operation has 3 warmups followed by 11 samples; the median
is reported. Rust and JS benchmarks were run sequentially. JS GC is requested
before each sample, outside the timed section; GC during a sample may still
contribute to it. Rust document destruction is outside the timed section.

## Implementation

JSON Lens uses its production `process` and `format_document` functions.
Documents of at least 32 KiB use Sonic 0.5.10 SIMD parsing with raw numbers,
then convert to the existing ordered serde_json value. Containers are
preallocated; arrays with at least 4096 elements are converted with Rayon.
Small inputs, invalid inputs and unsupported conversion cases use serde_json,
retaining its validation errors and recursion limit. The conversion avoids f64
and preserves big integers, negative zero and exponent/trailing-zero precision.

The default pool had **14 threads** on this machine. Thread pool startup is
excluded, as it completes before warmups. The 1-thread row sets
`RAYON_NUM_THREADS=1`; it still uses SIMD and the same conversion pipeline.
SIMD with one thread improved 20 MiB parsing by about 25% over the original
serde path in the corresponding run, but remained slower than V8. The larger
default gain combines SIMD, preallocation and CPU parallelism. It is a
throughput comparison with single-threaded JS, not a same-core-count comparison.

The original serde path is included in the Rust benchmark with the same
preserve_order/arbitrary_precision features and formatter. JS-native uses
`JSON.parse` / `JSON.stringify`. Microsoft's
[jsonc-parser](https://github.com/microsoft/node-jsonc-parser), a TypeScript library
used by VS Code, parses in strict mode (comments and trailing commas disabled),
then uses native JSON.stringify for serialization. Formatters indent by two
spaces. Cached formatting starts with an already-parsed value and remains
single-threaded in Rust. Cached JS serialization is shared by both JS variants.

## Results

| Input | Implementation | Parse | Parse + pretty | Parse + minify | Cached pretty | Cached minify |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 1 MiB | JSON Lens (14 threads) | 1.530 | 3.094 | 2.119 | 1.697 | 0.766 |
| 1 MiB | JSON Lens (1 thread) | 3.801 | 5.458 | 4.612 | 1.648 | 0.755 |
| 1 MiB | Original serde path | 5.044 | 6.656 | 5.890 | — | — |
| 1 MiB | JS native | 1.833 | 4.403 | 2.637 | 2.615 | 0.833 |
| 1 MiB | jsonc-parser | 16.848 | 21.413 | 20.207 | — | — |
| 5 MiB | JSON Lens (14 threads) | 5.996 | 14.758 | 10.748 | 10.089 | 5.204 |
| 5 MiB | JSON Lens (1 thread) | 19.612 | 28.535 | 24.529 | 8.778 | 4.521 |
| 5 MiB | Original serde path | 26.953 | 38.146 | 41.025 | — | — |
| 5 MiB | JS native | 9.297 | 24.471 | 13.519 | 14.719 | 4.240 |
| 5 MiB | jsonc-parser | 66.126 | 84.694 | 78.178 | — | — |
| 20 MiB | JSON Lens (14 threads) | 23.655 | 59.022 | 42.890 | 36.691 | 18.870 |
| 20 MiB | JSON Lens (1 thread) | 78.278 | 113.429 | 97.674 | 34.596 | 17.267 |
| 20 MiB | Original serde path | 106.198 | 146.051 | 130.215 | — | — |
| 20 MiB | JS native | 44.089 | 105.376 | 63.211 | 59.097 | 17.314 |
| 20 MiB | jsonc-parser | 278.665 | 344.362 | 319.466 | — | — |

All times are milliseconds. [Raw CSV](performance-results.csv).

The temporary Sonic DOM and the converted serde tree coexist during parsing,
so this optimization trades additional peak memory for throughput. Memory use
was not measured. The workload favors large arrays; large objects, deep nesting,
string-heavy or number-heavy documents can have different results.

The measurements exclude opening files, rendering, search, graph building and
browser responsiveness. No complete json4u / JSON Crack application was timed,
and Node.js results do not represent all browser engines. **The native Rust
results above are native results. WASM measurements are listed separately below.** Rust retains arbitrary-precision numbers; JS numbers
may lose precision outside this benchmark's range.

## WebAssembly results

Measured on the same machine with Chrome 156.0.8078.73 and Rust 1.95.0 release
on 2026-10-29. Both parsers run in the **same WASM build**, with `simd128`
enabled; browser tasks remain single-threaded. The Sonic path includes the
conversion to ordered serde_json values. The comparison uses the same JSON
arrays, 3 warmups and 11 samples, with generation, JS-to-WASM string transfer
and document destruction outside the timed section. DEBUG logging is disabled
while measuring. Browser timings use `performance.now` through web-time;
timer precision varies by browser. No compilation or other benchmark was run
concurrently with the recorded measurement.

| Input | serde parse | Sonic parse | serde parse + pretty | Sonic parse + pretty | serde parse + minify | Sonic parse + minify |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 MiB | 6.2 | 4.7 | 8.4 | 6.9 | 7.1 | 5.6 |
| 5 MiB | 30.5 | 23.0 | 41.9 | 34.2 | 35.5 | 27.8 |
| 20 MiB | 121.8 | 92.6 | 166.0 | 136.1 | 142.1 | 110.9 |

All times are milliseconds. [WASM raw CSV](performance-wasm-results.csv).
Sonic reduced parsing time by approximately 24–26% in this workload. These
results compare WASM processing paths, not browser-native JSON.parse or native
Rust performance. Numbers retain the same precision and objects retain the
same key/duplicate-key behavior. Twelve browser parity cases and the actual
editor/automatic Compare flow were checked.

The browser build enables Sonic with arbitrary-precision numbers, WASM SIMD128
and the browser getrandom backend through `.cargo/config.toml`. It requires
WASM SIMD support; modern Chrome, Firefox and Safari support it. Small inputs
and unsupported/invalid cases continue to use serde_json. Sonic calculations within the Worker use one thread. A separate UI thread
handles rendering, syntax highlighting and message encoding/decoding.

To reproduce, build the optional benchmark exports, serve `dist/`, and run
Chrome with a separate test profile and a remote-debugging port:

```sh
trunk build --release --features bench-web --public-url /json-lens/
# After hosting the build at http://127.0.0.1:8081/json-lens/:
node bench/browser.mjs http://127.0.0.1:9227 http://127.0.0.1:8081/json-lens/ /tmp/json-lens-browser
```

The script writes CSV results, environment metadata and DEBUG logs, checks
parser parity and exercises automatic comparison. Benchmark exports are
excluded from ordinary builds. Rebuild with `trunk build --release` after
measurement to return to the normal app.

## Web Worker responsiveness

The browser app now runs JSON processing, formatting, Sort/Expand, Graph
construction, source location/editing, automatic Compare and full-document
search in a dedicated module Worker. The UI owns fonts/layout, highlighting
and result application. Worker-side caches retain up to four parsed document
snapshots. Cached requests send document IDs; a cache miss requests the source
once for rehydration. Results return formatted text only when changed and reuse
an existing Graph when possible. ArrayBuffer transfer avoids an additional
structured-clone copy of each encoded message. Data trees stay in the Worker;
the UI validates JSON pointers asynchronously against the cached document.

A real 20 MiB JSON file import was checked in Chrome without COOP/COEP headers
(`crossOriginIsolated=false`). A 5 ms main-thread heartbeat ran **32 times**
while the posted Worker request was pending, with a **5.9 ms maximum gap**.
The postMessage-to-response interval was approximately **163 ms** and included
parsing, bounded Graph construction (10,000 cards), result encoding and return
transfer. This is one responsiveness smoke check, not a median throughput or
frame-rate benchmark. File reading, request preparation, result decoding and
rendering are outside that interval and still have costs on the main thread.

[Recorded check](worker-responsiveness.json).

The browser regression script checks precise numbers, cached formatting,
Sort/Expand, node edit/rename, source location, pointer validation, searches,
Compare, cache eviction/rehydration, a 20 MiB heartbeat, and actual UI formatting,
automatic comparison, JSON download, two-space indentation and formatting
undo/redo in both editors. Run against the ordinary build:

```sh
node test/browser-worker.mjs http://127.0.0.1:9227 http://127.0.0.1:8081/json-lens/
```

The test expects an isolated Chrome session with remote debugging and saves
results/logs under `/tmp`. DEBUG traces distinguish `realm=main` and
`realm=worker`; see [diagnostics](debug.md). Worker startup/runtime failures are
reported to the app and outstanding jobs are completed with errors.

## Browser editor responsiveness

Measured in the actual release WASM app on the same machine and Chrome version,
with a 1280 × 800 viewport, device scale factor 1, and DEBUG logging enabled.
These ASCII JSON arrays contain approximately one million characters and are
below the full-editing limit: the editor displays the complete document.
One formatted file has 999,939 characters; the compact file has 987,798.

| Input | Maximum frame gap while opening, before → after | Median scroll frame gap, before → after | Scroll P95, before → after |
| --- | ---: | ---: | ---: |
| Formatted JSON | 416.7 → 116.7 ms | 83.3 → 16.7 ms | 100.0 → 33.3 ms |
| Compact JSON | 350.0 → 166.6 ms | 50.0 → 16.7 ms | 50.1 → 33.4 ms |

[Recorded UI checks](performance-ui-results.json). `requestAnimationFrame`
gaps were recorded through the Open-file flow and sixteen mouse-wheel events.
The opening number is the longest main-thread/frame stall during a 2.5-second
observation window, not total file-loading time. These are smoke observations
for two inputs in an isolated browser, not universal frame-rate guarantees.
Cold layout still costs main-thread time; no peak-memory measurement was made.

The bottleneck was repeated editor layout: the formatted case logged approximately
82 ms per cached layout before the change, versus about 19 ms for the largest
Worker result decode. The editor now builds only the requested wrapped layout,
coalesces adjacent syntax sections of the same color, caches unchanged Compare
alignment, and stops gutter painting once rows pass the viewport. It still
resolves layouts through egui's font cache so theme/font-atlas changes remain
correct. Full source, soft-wrap, editing, precision and undo are preserved.

To reproduce against an ordinary release build with the preview URL and an
isolated Chrome remote-debugging session on port 9227:

```sh
node bench/ui-browser.mjs after pretty
node bench/ui-browser.mjs after minified
```

The script generates its input files and writes JSON metrics and DEBUG logs
under `/private/tmp`. A separate native CPU benchmark exercises the actual
workspace and tessellation in Graph and Compare, excluding window/GPU rendering:

```sh
cargo run --release --locked --features bench-ui -- --bench-editor
```

## Reproduce processing measurements

From the repository root, with Rust and Node.js installed:

```sh
cargo bench --bench comparison --locked
RAYON_NUM_THREADS=1 cargo bench --bench comparison --locked
npm install --prefix /tmp/json-lens-comparison --no-audit --no-fund jsonc-parser@3.3.1
node --expose-gc bench/comparison.mjs /tmp/json-lens-comparison
```

Sources: [Rust](../bench/comparison.rs), [JS/TS libraries](../bench/comparison.mjs).
The original processing and graph benchmarks remain available:

```sh
cargo bench --bench processing --locked
cargo bench --bench workspace --locked
```
