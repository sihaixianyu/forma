"""Summarize native stderr or browser Console DEBUG logs without dependencies."""
import argparse
import collections
import math
import re
import statistics

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("log", help="Saved DEBUG log file")
parser.add_argument("--job", help="Only include one job ID")
parser.add_argument("--realm", choices=["main", "worker", "native"], help="Only include one execution realm")
args = parser.parse_args()
stages = collections.defaultdict(list)
paths = collections.Counter()
queues = []
jobs = collections.defaultdict(list)
with open(args.log, encoding="utf-8") as stream:
    for line in stream:
        if "[JSON Lens DEBUG]" not in line:
            continue
        fields = dict(re.findall(r"(\w+)=([^\s]+)", line))
        if args.job is not None and fields.get("job") != args.job:
            continue
        if args.realm is not None and fields.get("realm", "native") != args.realm:
            continue
        if fields.get("event") == "stage":
            name = fields.get("stage")
            value = float(fields["elapsed_ms"])
            stages[(fields.get("realm", "native"), name)].append(value)
            if name == "worker_job":
                jobs[fields.get("job", "0")].append(value)
        elif fields.get("event") == "parser_path":
            paths[(fields.get("parser"), fields.get("reason", "fast_path"))] += 1
        elif fields.get("event") == "job_dequeued":
            queues.append(float(fields["queue_ms"]))

def p95(values):
    return sorted(values)[math.ceil(len(values) * .95) - 1]

print("realm,stage,count,total_ms,median_ms,p95_ms,max_ms")
for (realm, name), values in sorted(stages.items(), key=lambda item: sum(item[1]), reverse=True):
    print(f"{realm},{name},{len(values)},{sum(values):.3f},{statistics.median(values):.3f},{p95(values):.3f},{max(values):.3f}")
if queues:
    print(f"Queue: median={statistics.median(queues):.3f}ms p95={p95(queues):.3f}ms max={max(queues):.3f}ms")
for (path, reason), count in paths.items():
    print(f"Parser: {path}, reason={reason}, count={count}")
for job, values in sorted(jobs.items(), key=lambda item: max(item[1]), reverse=True)[:5]:
    print(f"Slow job: id={job}, elapsed_ms={max(values):.3f}")
print("Stage times include nested stages; do not add parent and child totals.")
