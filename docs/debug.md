# DEBUG performance logs

Logs connect each queued job to its processing stages using `job=<id>`.
They show which parser ran, whether formatting reused a graph, where time was
spent, and the caret → source offset → JSON pointer → Path/Graph sequence. They
contain sizes, offsets, JSON pointers and operation labels, without document
values or filenames. Debug builds automatically enable only the low-volume
navigation trace; high-frequency timing logs remain opt-in.

## Enable

Native debug builds truncate `forma-debug.log` at startup and write only
caret/Path/Graph navigation events there:

```sh
cargo run
tail -f forma-debug.log
```

Set `FORMA_LOG_FILE=/path/to/file` to choose another local file. To additionally
enable the full timing stream in a release or debug build:

```sh
FORMA_LOG=debug cargo run --release --locked 2> debug.log
```

Browser debug builds log the low-volume navigation trace automatically. For a
release build, open the app with `?debug=1`, then open DevTools Console and enable **Verbose**. Save the Console
output to a local file for analysis. Reload without the query parameter to
disable DEBUG in release builds.

For local Web debugging that an agent can monitor directly, use:

```sh
python3 scripts/debug-web.py
tail -f forma-debug.log
```

The helper binds a log sink to loopback port `8081`, starts `trunk serve` on its
configured port, and writes only the low-volume navigation trace. It does not
receive JSON contents or property values.

Example fields:

```text
[Forma DEBUG] job=4 event=job_dequeued queue_ms=4.300
[Forma DEBUG] job=4 event=work_metadata revision=2 user_action=false reset_view=false
[Forma DEBUG] job=4 event=parser_path parser=sonic simd128=true parallel=false
[Forma DEBUG] job=4 event=stage stage=parse_sonic bytes=52835 elapsed_ms=0.100
[Forma DEBUG] job=4 event=compare_result differences=1
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
| `source_caret`, `source_pointer_queued`, `source_pointer_resolved` | Caret byte offset and the pointer lookup lifecycle |
| `source_pointer_result`, `path_updated` | Stale-result rejection and the Path that was actually applied |
| `focused_graph_queued`, `focused_graph_result` | Local Graph request and whether its result still matched the active Path |
| `compare_walk`, `compare_alignment` | Finding value differences and aligning text rows |
| `search_text`, `search_tree` | Source and graph search |
| `search_highlights` | Main-thread visible-match painting and row indexing |
| `syntax_layout` | Syntax layout cache misses |
| `editor_layout` | Editor layout calls exceeding 8 ms; fast calls are omitted |
| `job_coalesced`, `queue_coalesced`, `stale_result` | Superseded work and results discarded after editing |
| `execute_job`, `worker_job`, `process`, `parse_json` | Inclusive totals containing other stages |

`event=graph_view` reports total nodes, visible nodes and visible edges when the
graph viewport changes. Invalid navigation paths keep the graph visible; after
a document update removes the selected path, navigation returns to the root.
`event=graph_view_recovered` identifies recovery from an empty viewport.
`operation="Focus graph"` builds a local view for a pointer omitted from the
initial preview; this work runs off the UI thread on both platforms.

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
FORMA_LOG=debug cargo bench --bench processing --locked 2> debug.log
```

For measured browser comparisons and automatic log capture, see the
[WASM benchmark](performance.md#webassembly-results).
