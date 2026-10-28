# SPDX-License-Identifier: GPL-3.0-or-later
"""Hash-checked, sequential M0.7E SQLx comparisons; generate fixtures beforehand."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess
from datetime import datetime, timezone


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--results", type=Path, required=True)
    parser.add_argument("--database", action="append", required=True, help="label=absolute path")
    parser.add_argument("--iterations", type=int, default=30)
    parser.add_argument("--passes", type=int, default=2)
    parser.add_argument("--strategies", choices=["all", "finalists", "relationships"], default="relationships")
    args = parser.parse_args()
    if not 1 <= args.iterations <= 10000 or not 1 <= args.passes <= 10:
        parser.error("Invalid iteration/pass bounds")
    binary = args.executable.resolve(strict=True)
    datasets = []
    for value in args.database:
        label, path = value.split("=", 1)
        if not label or any(c not in "abcdefghijklmnopqrstuvwxyz0123456789-_" for c in label):
            parser.error("Use simple lowercase dataset labels")
        path = Path(path)
        if not path.is_absolute():
            parser.error("Explicit absolute database paths required")
        path = path.resolve(strict=True)
        datasets.append({"label": label, "path": str(path), "sha256": digest(path), "bytes": path.stat().st_size})
    if len({d["label"] for d in datasets}) != len(datasets):
        parser.error("Duplicate labels")
    root = args.results.absolute()
    root.mkdir(parents=True, exist_ok=False)
    manifest = {"utc": datetime.now(timezone.utc).isoformat(), "platform": platform.platform(), "commit": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(), "binary": str(binary), "binarySha256": digest(binary), "datasets": datasets, "iterations": args.iterations, "passes": args.passes, "strategies": args.strategies, "cache": "Retained OS caches. No concurrent builds/tests. Each shape warmed; strategy order reverses each round; dataset order reverses each pass.", "completed": []}
    def save():
        (root / "environment.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    save()
    for repeat in range(args.passes):
        for dataset in (datasets if repeat % 2 == 0 else list(reversed(datasets))):
            path = Path(dataset["path"])
            assert digest(path) == dataset["sha256"], "Input changed before measurement"
            output = root / f"{dataset['label']}-pass-{repeat}.json"
            print("Measuring", output.name, flush=True)
            with output.open("x", encoding="utf-8") as out, output.with_suffix(".stderr.txt").open("x", encoding="utf-8") as err:
                subprocess.run([str(binary), "query-diagnose", "--database", str(path), "--iterations", str(args.iterations), "--strategies", args.strategies], stdout=out, stderr=err, check=True)
            assert digest(path) == dataset["sha256"], "Read-only input changed"
            report = json.loads(output.read_text(encoding="utf-8-sig"))
            manifest["completed"].append({"output": output.name, "shapes": len(report["scenarios"]), "sqlite": report["sqlite"]})
            save()
            print(output.name, "passed", flush=True)
    assert digest(binary) == manifest["binarySha256"], "Executable changed"


if __name__ == "__main__":
    main()
