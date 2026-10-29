# Processing benchmarks

Measured locally on 2026-10-29: Apple M4 Pro, macOS 27.2 / arm64,
Rust 1.95.0 release (thin LTO), Node.js 26.12.1 (V8), jsonc-parser 3.3.1.

The same compact JSON array is generated in both benchmarks. Rows contain a
Unicode string, a boolean, a small integer and a four-element numeric array.
All numbers are within JS's exact integer range. Input generation and file I/O
are excluded. Each operation has 3 warmups followed by 11 samples; the median
is reported. JS GC is requested before each sample, outside the timed section;
GC during a sample may still contribute to it. Rust document destruction is
outside the timed section.

JSON Lens uses its production `process` and `format_document` functions with
serde_json's preserve_order and arbitrary_precision features. JS-native uses
`JSON.parse` / `JSON.stringify`. Microsoft's
[jsonc-parser](https://github.com/microsoft/node-jsonc-parser), a TypeScript library
used by VS Code, parses in strict mode (comments and trailing commas disabled),
then uses native JSON.stringify for serialization. Both formatters indent by two
spaces. Cached formatting starts with an already-parsed value.

## Results

| Input | Implementation | Parse | Parse + pretty | Parse + minify | Cached pretty | Cached minify |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 1 MiB | JSON Lens | 4.914 | 6.494 | 5.649 | 1.519 | 0.702 |
| 1 MiB | JS native | 1.840 | 4.229 | 2.559 | 2.486 | 0.745 |
| 1 MiB | jsonc-parser | 15.932 | 20.839 | 20.569 | — | — |
| 5 MiB | JSON Lens | 24.895 | 34.038 | 29.959 | 8.586 | 4.630 |
| 5 MiB | JS native | 9.246 | 23.829 | 13.271 | 14.413 | 4.022 |
| 5 MiB | jsonc-parser | 64.214 | 79.876 | 76.704 | — | — |
| 20 MiB | JSON Lens | 99.678 | 136.466 | 120.238 | 35.154 | 19.293 |
| 20 MiB | JS native | 43.625 | 103.526 | 60.769 | 57.657 | 16.276 |
| 20 MiB | jsonc-parser | 253.683 | 320.044 | 305.225 | — | — |

All times are milliseconds. Cached serialization is shared by the two JS
variants, so it is listed once. [Raw CSV](performance-results.csv).

The workload is synthetic and hardware-dependent. It does not measure opening
files, rendering, memory use, search, graph building or browser responsiveness.
No complete json4u / JSON Crack application was timed, and Node.js results do not
claim to represent all browser engines. The native Rust build is measured;
WebAssembly timings are not included. Rust retains arbitrary-precision JSON
numbers; native JS numbers may lose precision outside this benchmark's range.

## Reproduce

From the repository root, with Rust and Node.js installed:

```sh
cargo bench --bench comparison --locked
npm install --prefix /tmp/json-lens-comparison --no-audit --no-fund jsonc-parser@3.3.1
node --expose-gc bench/comparison.mjs /tmp/json-lens-comparison
```

Sources: [Rust](../bench/comparison.rs), [JS/TS libraries](../bench/comparison.mjs).
The original processing and graph benchmarks remain available:

```sh
cargo bench --bench processing --locked
cargo bench --bench workspace --locked
```
