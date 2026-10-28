# SPDX-License-Identifier: GPL-3.0-or-later
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("summary", Path(__file__).with_name("summarize-relationships.py"))
summary = importlib.util.module_from_spec(spec)
spec.loader.exec_module(summary)


class SummaryTests(unittest.TestCase):
    def test_nearest_rank_and_missing_samples(self):
        self.assertEqual(summary.distribution([4, 1, 3, 2]), {"n": 4, "p50": 2, "p95": 4, "max": 4})
        with self.assertRaises(ValueError):
            summary.distribution([])

    def test_pool_raw_samples_and_preserve_passes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            files = [f"reference-pass-{i}.json" for i in range(2)]
            (root / "environment.json").write_text(json.dumps({"completed": [{"output": f} for f in files]}))
            for i, file in enumerate(files):
                cases = []
                for strategy, samples in [("OrderedEntrySet", [10, 20 + i]), ("PageMembership", [5, 10 + i])]:
                    cases.append({"name": "tail", "strategy": strategy, "count": 123, "request": {"offset": 23}, **{phase: {"samples": samples} for phase in ["countMs", "pageMs", "combinedMs", "selectionMs", "projectionMs", "poolMs"]}})
                (root / file).write_text(json.dumps({"scenarios": cases}))
            result = summary.summarize(root)
            self.assertEqual(len(result["passes"]), 4)
            candidate = next(r for r in result["pooled"] if r["strategy"] == "PageMembership")
            self.assertEqual(candidate["phases"]["pageMs"], {"n": 4, "p50": 5, "p95": 11, "max": 11})
            self.assertAlmostEqual(candidate["saved"]["pageMs"]["p95"]["ms"], 10)


if __name__ == "__main__":
    unittest.main()
