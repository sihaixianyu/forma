# DEBUG performance logs

Logs connect each queued job to its processing stages using `job=<id>`.
They show which parser ran, whether formatting reused a graph, and where time
was spent. They contain sizes, counts and operation labels, without document
contents, property values or filenames. DEBUG is disabled by default.

## Enable

Native logs go to stderr:

```sh
JSON_LENS_LOG=debug cargo run --release --locked 2> debug.log
```

Browser: open the app with `?debug=1`, then open DevTools Console and enable
**Verbose**. Save the Console output to a local file for analysis. Reload
without the query parameter to disable DEBUG.

Example fields:

```text
[JSON Lens DEBUG] job=4 event=job_dequeued queue_ms=4.300
[JSON Lens DEBUG] job=4 event=work_metadata revision=2 user_action=false reset_view=false
[JSON Lens DEBUG] job=4 event=parser_path parser=sonic simd128=true parallel=false
[JSON Lens DEBUG] job=4 event=stage stage=parse_sonic bytes=52835 elapsed_ms=0.100
[JSON Lens DEBUG] job=4 event=compare_result differences=1
```

## Analyze

```sh
python3 bench/analyze-log.py debug.log
python3 bench/analyze-log.py debug.log --job 4
python3 bench/analyze-log.py debug.log --realm worker
```

The dependency-free script reports count, total, median, P95 and maximum wall
time by stage, queue delay, parser paths/fallback reasons, and the slowest jobs.
`job=0` means work outside a queued job, including editor layout. Browser logs
include `realm=main` or `realm=worker`, so formatting/processing can be separated
from UI layout and message handling. The analyzer groups stages by realm.

| Field or stage | What it explains |
| --- | --- |
| `queue_ms` | Time from mailbox submission until execution, including browser scheduling |
| `worker_encode`, `worker_decode`, `worker_result_encode` | Request encoding on UI, result decoding on UI, and response encoding in Worker |
| `worker_roundtrip` | Queue/submission to reply receipt, before typed result decoding/rendering |
| `worker_cache_retry` | Cache miss; source is resent once to restore the Worker document |
| `work_metadata` | Source revision, user action versus automatic update |
| `parse_sonic`, `parse_serde` | Time spent in each parser |
| `convert_dom` | Converting Sonic values to the app's ordered serde values |
| `parser_path` | Sonic versus serde; small input, parse failure or unsupported conversion |
| `transform`, `serialize` | Sort/Expand and JSON output generation |
| `graph_build`, `graph_cache` | Graph construction versus reuse |
| `compare_walk`, `compare_alignment` | Finding value differences and aligning text rows |
| `search_text`, `search_tree` | Source and graph search |
| `syntax_layout` | Syntax layout cache misses |
| `editor_layout` | Editor layout calls exceeding 8 ms; fast calls are omitted |
| `job_coalesced`, `queue_coalesced`, `stale_result` | Superseded work and results discarded after editing |
| `execute_job`, `worker_job`, `process`, `parse_json` | Inclusive totals containing other stages |

Start with a slow job ID, then compare its child stages. A large `queue_ms`
with short processing suggests scheduling/backlog. A large `convert_dom`
points to data-tree conversion; a large `compare_alignment` points to text
alignment. Repeated fallbacks show when Sonic was not used.

Stage wall times include nested stages, so **do not add parent and child totals**.
DEBUG output itself affects inclusive timings. Use logging to locate the
bottleneck, then disable it for repeatable benchmarks. Logs do not measure
GPU execution, frame presentation, memory allocations or CPU samples; browser
Performance/Memory tools are needed for those measurements. With DEBUG off,
stage timers do not read the clock or allocate log messages.

Native processing logs can also be generated without opening a GUI:

```sh
JSON_LENS_LOG=debug cargo bench --bench processing --locked 2> debug.log
```

For measured browser comparisons and automatic log capture, see the
[WASM benchmark](performance.md#webassembly-results).
