// SPDX-License-Identifier: GPL-3.0-or-later
import { createRequire } from "node:module";
import assert from "node:assert/strict";
import test from "node:test";
import { observeArrival, watchTarget } from "./arrival.mjs";
const require = createRequire(
  new URL("../apps/desktop/package.json", import.meta.url),
);
const { JSDOM } = require("jsdom");
function fixture() {
  const dom = new JSDOM(
    `<main class="main-content"><section class="library-panel" data-query-key="new" data-query-ready="true"><div class="library-scroll"><div id="g1" class="game-row" aria-rowindex="2" aria-busy="true"></div></div></section></main>`,
    { runScripts: "outside-only", pretendToBeVisual: true },
  );
  const w = dom.window;
  for (const e of w.document.querySelectorAll("div"))
    e.getBoundingClientRect = () => ({ top: 0, bottom: 38 });
  w.eval(`window.arrival = ${observeArrival.toString()}`);
  w.eval(`window.targetArrival = ${watchTarget.toString()}`);
  return { dom, w, row: w.document.querySelector(".game-row") };
}
test("waits for loaded row and a matching generation, then records frame opportunities", async () => {
  const { dom, w, row } = fixture();
  try {
    const promise = w.arrival(0, "new", () => {
      w.document.querySelector(".library-panel").dataset.queryKey = "old";
      row.setAttribute("aria-busy", "false");
      setTimeout(() => {
        w.document.querySelector(".library-panel").dataset.queryKey = "new";
      }, 20);
    });
    const r = await promise;
    assert.equal(r.id, "g1");
    assert(
      r.domMs >= 15 &&
        r.secondFrameMs >= r.nextFrameMs &&
        r.nextFrameMs >= r.domMs,
    );
  } finally {
    dom.window.close();
  }
});
test("rejects offscreen and unloaded rows instead of recording false arrivals", async () => {
  const { dom, w, row } = fixture();
  try {
    row.getBoundingClientRect = () => ({ top: 100, bottom: 138 });
    await assert.rejects(
      w.arrival(0, "new", () => row.setAttribute("aria-busy", "false"), 40),
      /timed out/,
    );
  } finally {
    dom.window.close();
  }
});
test("optional 50-ms observation control preserves the earlier fine arrival", async () => {
  const { dom, w, row } = fixture();
  try {
    const r = await w.arrival(
      0,
      "new",
      () => {
        setTimeout(() => row.setAttribute("aria-busy", "false"), 10);
      },
      1000,
      true,
    );
    assert(r.coarseMs >= r.domMs);
    assert(r.secondFrameMs >= r.domMs);
  } finally {
    dom.window.close();
  }
});
test("target arrival can precede complete-window readiness without weakening its guard", async () => {
  const { dom, w, row } = fixture();
  try {
    w.document.querySelector(".library-panel").dataset.queryReady = "false";
    const target = w.targetArrival(0, "new");
    const full = w.arrival(0, "new", () => {
      row.setAttribute("aria-busy", "false");
      setTimeout(() => {
        w.document.querySelector(".library-panel").dataset.queryReady = "true";
      }, 25);
    });
    const t = await target.promise;
    assert.equal(t.id, "g1");
    assert.equal(
      w.document.querySelector(".library-panel").dataset.queryReady,
      "false",
    );
    const r = await full;
    assert.equal(r.id, t.id);
    assert(r.domMs >= 15);
    target.cancel();
  } finally {
    dom.window.close();
  }
});
