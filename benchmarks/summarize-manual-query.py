# SPDX-License-Identifier: GPL-3.0-or-later
"""Assert v1/v2 projection equivalence and summarize finite warm native samples."""
import json
import math
from pathlib import Path
import statistics
import sys

root = Path(sys.argv[1])
summary = []
for size in (15000, 25000):
    runs = {}
    for version in (1, 2):
        runs[version] = [json.loads((root / f"{size}-{p}-v{version}/query.json").read_text(encoding="utf-8")) for p in (0, 1)]
    for p in (0, 1):
        assert runs[1][p]["status"]["counts"] == runs[2][p]["status"]["counts"]
        for before, after in zip(runs[1][p]["results"], runs[2][p]["results"], strict=True):
            for field in ("name", "offset", "total", "ids", "rows"):
                assert before[field] == after[field], (size, p, field)
    for index, case in enumerate(runs[1][0]["results"]):
        row = {"games": size, "case": case["name"], "offset": case["offset"], "total": case["total"]}
        for version in (1, 2):
            samples = [s for run in runs[version] for s in run["results"][index]["samples"]]
            stats = {}
            for field in ("queryMs", "invokeMs"):
                values = sorted(s[field] for s in samples)
                stats[field] = {"n": len(values), "median": statistics.median(values), "p95": values[math.ceil(len(values) * .95)-1], "max": max(values)}
            row[f"v{version}"] = stats
        summary.append(row)
print(json.dumps({"equivalent": True, "cases": summary}, indent=2))
