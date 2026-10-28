# SPDX-License-Identifier: GPL-3.0-or-later
"""Summarize raw M0.7E samples without adding/subtracting percentiles."""
import argparse
from collections import defaultdict
import json
import math
from pathlib import Path


def distribution(samples):
    values = sorted(samples)
    if not values:
        raise ValueError("Missing samples")
    return {"n": len(values), "p50": values[math.ceil(len(values) * .5) - 1], "p95": values[math.ceil(len(values) * .95) - 1], "max": values[-1]}


def summarize(root):
    environment = json.loads((root / "environment.json").read_text(encoding="utf-8-sig"))
    groups = defaultdict(lambda: defaultdict(list))
    passes = []
    for completed in environment["completed"]:
        path = root / completed["output"]
        label = path.name.split("-pass-")[0]
        report = json.loads(path.read_text(encoding="utf-8-sig"))
        for case in report["scenarios"]:
            phases = {phase: case[phase]["samples"] for phase in ["countMs", "pageMs", "combinedMs", "selectionMs", "projectionMs", "poolMs"]}
            key = (label, case["name"], case["strategy"])
            for phase, samples in phases.items():
                groups[key][phase].extend(samples)
            passes.append({"file": path.name, "dataset": label, "name": case["name"], "strategy": case["strategy"], "count": case["count"], "offset": case["request"]["offset"], "phases": {phase: distribution(samples) for phase, samples in phases.items()}})
    rows = [{"dataset": key[0], "name": key[1], "strategy": key[2], "phases": {phase: distribution(samples) for phase, samples in phases.items()}} for key, phases in sorted(groups.items())]
    current = {(row["dataset"], row["name"]): row for row in rows if row["strategy"] == "OrderedEntrySet"}
    for row in rows:
        baseline = current.get((row["dataset"], row["name"]))
        if baseline:
            row["saved"] = {phase: {stat: {"ms": baseline["phases"][phase][stat] - data[stat], "percent": 100 * (baseline["phases"][phase][stat] - data[stat]) / baseline["phases"][phase][stat]} for stat in ["p50", "p95", "max"]} for phase, data in row["phases"].items()}
    return {"note": "Nearest-rank statistics pooled from raw samples; process/pass observations remain separate. Split diagnostic queries are not components of full-page time.", "pooled": rows, "passes": passes}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    args = parser.parse_args()
    print(json.dumps(summarize(args.root), indent=2))
