// SPDX-License-Identifier: GPL-3.0-or-later
import { test } from "node:test";
import assert from "node:assert/strict";
import { derive, distribution } from "./startup.mjs";
const report = {
  scenario: "startup",
  timeOrigin: 1100,
  initial: { firstUsableMs: 100, total: 500, visibleRows: 18 },
  startup: { firstUsable: 200, shellCommit: [50], visibleThumbnailReady: 0 },
};
const external = {
  createdEpochMs: 1000,
  visibleLowerMs: 10,
  visibleUpperMs: 30,
  profile: "reused",
};
test("median averages central pair; nearest rank retains the maximum tail", () => {
  assert.deepEqual(distribution([4, 1, 3, 2]), {
    n: 4,
    median: 2.5,
    p95: 4,
    max: 4,
  });
  assert.equal(distribution([]), null);
});
test("complete process boundary and visibility interval are preserved", () => {
  const r = derive(report, external, []);
  assert.equal(r.processToRows, 300);
  assert.equal(r.processToShell, 150);
  assert.equal(r.visibleToRowsLower, 270);
  assert.equal(r.visibleToRowsUpper, 290);
  assert.equal(r.processToImages, null);
  assert.equal(r.databaseTask, null);
});
test("native phases use monotonic clock even with a UTC clock adjustment", () => {
  const r = derive(report, external, [
    { startupStage: "runtime-built", epochMs: 1200, elapsedMs: 10 },
    { startupStage: "setup-entry", epochMs: 1000, elapsedMs: 910 },
  ]);
  assert.equal(r.runtimeToSetup, 900);
});
test("historical UTC-only phase provenance remains supported", () => {
  assert.equal(
    derive(report, external, [
      { startupStage: "runtime-built", epochMs: 1100 },
      { startupStage: "setup-entry", epochMs: 1900 },
    ]).runtimeToSetup,
    800,
  );
});
test("invalid, failed and missing usable-row observations are rejected", () => {
  assert.throws(() => derive({ ...report, failure: "no rows" }, external, []));
  assert.throws(() => derive({ ...report, startup: {} }, external, []));
  assert.throws(() => derive(report, { ...external, visibleUpperMs: 0 }, []));
});

test("later metadata probes cannot overwrite initial startup phases", () => {
  const marks = [
    { startupStage: "metadata-request-entry", epochMs: 1200, elapsedMs: 200 },
    {
      startupStage: "metadata-request-complete",
      epochMs: 1205,
      elapsedMs: 205,
    },
    { startupStage: "metadata-request-entry", epochMs: 3000, elapsedMs: 2000 },
    {
      startupStage: "metadata-request-complete",
      epochMs: 3050,
      elapsedMs: 2050,
    },
  ];
  assert.equal(derive(report, external, marks).metadata, 5);
});

test("an empty or placeholder-only startup cannot satisfy the row benchmark", () => {
  assert.throws(() =>
    derive(
      { ...report, initial: { ...report.initial, total: 0 } },
      external,
      [],
    ),
  );
  assert.throws(() =>
    derive(
      { ...report, initial: { ...report.initial, visibleRows: 0 } },
      external,
      [],
    ),
  );
});

test("a visible placeholder cohort without a usable-row mark is rejected", () => {
  assert.throws(() =>
    derive(
      { ...report, initial: { ...report.initial, firstUsableMs: null } },
      external,
      [],
    ),
  );
});

test("a buffered duplicate adds monotonic provenance to a legacy mark", () => {
  const r = derive(report, external, [
    { startupStage: "database-task-entry", epochMs: 1100, elapsedMs: 10 },
    { startupStage: "backend-published", epochMs: 1200 },
    { startupStage: "backend-published", epochMs: 1200, elapsedMs: 110 },
  ]);
  assert.equal(r.databaseTask, 100);
});
