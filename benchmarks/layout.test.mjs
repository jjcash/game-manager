// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { distribution, traceSummary } from "./summarize-layout.mjs";

test("nearest-rank tails preserve a rare long interval", () => {
  assert.deepEqual(distribution([10, 10, 10, 10, 235]), {
    n: 5,
    p50: 10,
    p95: 235,
    p99: 235,
    max: 235,
  });
  assert.equal(distribution([]), null);
});
test("layout totals use the marked renderer thread, keep CPU separate, and do not add nested JS", () => {
  const s = traceSummary([
    { name: "gm:sortTrial:0:baseline", pid: 1, tid: 2, ph: "R" },
    { name: "FunctionCall", pid: 1, tid: 2, ph: "X", dur: 90000, tdur: 85000 },
    {
      name: "Layout",
      pid: 1,
      tid: 2,
      ph: "X",
      dur: 70000,
      tdur: 60000,
      args: { beginData: { dirtyObjects: 255, totalObjects: 412 } },
    },
    { name: "Layout", pid: 1, tid: 2, ph: "X", dur: 2000, tdur: 1500 },
    { name: "Layout", pid: 9, tid: 9, ph: "X", dur: 999000, tdur: 999000 },
  ]);
  assert.equal(s.layoutWallMs, 72);
  assert.equal(s.layoutCpuMs, 61.5);
  assert.equal(s.functionWallMs, 90);
  assert.equal(s.layoutMaxMs, 70);
  assert.equal(s.dirtyMax, 255);
  assert.equal(s.objectsMax, 412);
});
test("absent thread CPU instrumentation is unavailable rather than zero", () => {
  assert.equal(
    traceSummary([{ name: "Layout", pid: 1, tid: 1, ph: "X", dur: 5000 }])
      .layoutCpuMs,
    null,
  );
  assert.equal(traceSummary([]).layoutWallMs, null);
});
