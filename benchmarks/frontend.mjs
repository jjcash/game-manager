// SPDX-License-Identifier: GPL-3.0-or-later
// Drive a real native WebView2 with explicit loopback remote debugging enabled.
// No browser-only backend mocks or HTTP data servers are involved.
import { writeFile } from "node:fs/promises";
import assert from "node:assert/strict";
const [endpoint = "http://127.0.0.1:9223", output] = process.argv.slice(2);
assert.equal(new URL(endpoint).hostname, "127.0.0.1");
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let targets;
const matches = (p) =>
  p.type === "page" &&
  /^(http:\/\/tauri\.localhost\/|http:\/\/127\.0\.0\.1:1420\/)/.test(p.url);
for (let i = 0; i < 100; i++) {
  try {
    targets = await (await fetch(`${endpoint}/json/list`)).json();
    if (targets.some(matches)) break;
  } catch {
    /* starting */
  }
  await sleep(100);
}
const target = targets?.find(matches);
assert(target, "Expected a Game Manager WebView page");
const socket = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((r, reject) => {
  socket.onopen = r;
  socket.onerror = reject;
});
let sequence = 0;
const pending = new Map();
socket.onmessage = (event) => {
  const m = JSON.parse(event.data);
  if (m.id) {
    const p = pending.get(m.id);
    pending.delete(m.id);
    if (m.error) p.reject(new Error(JSON.stringify(m.error)));
    else p.resolve(m.result);
  }
};
function command(method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = ++sequence;
    pending.set(id, { resolve, reject });
    socket.send(JSON.stringify({ id, method, params }));
  });
}
async function evaluate(expression) {
  const r = await command("Runtime.evaluate", {
    expression,
    returnByValue: true,
    awaitPromise: true,
  });
  if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails));
  return r.result.value;
}
async function until(expression) {
  for (let i = 0; i < 200; i++) {
    const r = await evaluate(`Boolean(${expression})`);
    if (r) return r;
    await sleep(50);
  }
  throw new Error(`Timed out: ${expression}`);
}
const click = (label) =>
  evaluate(
    `Array.from(document.querySelectorAll('button')).find(b=>b.textContent.trim()===${JSON.stringify(label)})?.click()`,
  );
const loaded = `document.querySelector('.game-row:not([aria-busy="true"])') && !document.querySelector('.library-scroll[aria-busy="true"]')`;
const snapshot = () =>
  evaluate(
    `JSON.parse(document.querySelector('[aria-label="Performance measurements"]').textContent)`,
  );
async function key(name, code) {
  await command("Input.dispatchKeyEvent", {
    type: "keyDown",
    key: name,
    code: name,
    windowsVirtualKeyCode: code,
  });
  await command("Input.dispatchKeyEvent", {
    type: "keyUp",
    key: name,
    code: name,
    windowsVirtualKeyCode: code,
  });
}
const report = {
  timestamp: new Date().toISOString(),
  url: target.url,
  scenarios: {},
};
try {
  await until(
    `document.querySelector('.diagnostics') || document.querySelector('[role="alert"]')`,
  );
  if (await evaluate(`!!document.querySelector('[role="alert"]')`)) {
    report.error = await evaluate(
      `document.querySelector('[role="alert"]').textContent`,
    );
    await click("Retry library");
    await until(`document.querySelector('[role="alert"]')`);
    report.retryError = await evaluate(
      `document.querySelector('[role="alert"]').textContent`,
    );
  } else {
    await until(loaded);
    await evaluate(`document.querySelector('.diagnostics').open=true`);
    await sleep(1100);
    report.initial = await snapshot();
    const total = report.initial.total;
    await click("Start frame capture");
    const start = performance.now();
    const bounds = await evaluate(
      `(()=>{const r=document.querySelector('.library-scroll').getBoundingClientRect();return {x:r.x+100,y:r.y+100}})()`,
    );
    for (let i = 0; i < 25; i++) {
      await command("Input.dispatchMouseEvent", {
        type: "mouseWheel",
        ...bounds,
        deltaX: 0,
        deltaY: 38,
      });
      await sleep(70);
    }
    await until(loaded);
    report.scenarios.slowWheel = {
      durationMs: performance.now() - start,
      scrollTop: await evaluate(
        `document.querySelector('.library-scroll').scrollTop`,
      ),
    };
    await evaluate(
      `(async()=>{const e=document.querySelector('.library-scroll');for(let i=0;i<100;i++){e.scrollTop=((i*7919)%${total})*38;await new Promise(requestAnimationFrame);} })()`,
    );
    await until(loaded);
    await sleep(1100);
    report.scenarios.rapid = await snapshot();
    // Wait for actual data at 20 distributed windows, rather than counting a
    // selected loading placeholder as proof that the destination is usable.
    for (let i = 0; i < 20; i++) {
      const index = Math.floor((i * (total - 10)) / 19);
      await evaluate(
        `document.querySelector('.library-scroll').scrollTop=${index * 38}`,
      );
      await until(
        `document.querySelector('.game-row[aria-rowindex="${index + 2}"][aria-busy="false"]')`,
      );
    }
    await sleep(1100);
    report.scenarios.settledWindows = await snapshot();
    await evaluate(`document.querySelector('[role="grid"]').focus()`);
    await key("End", 35);
    await until(
      `document.querySelector('.game-row[aria-rowindex="${total + 1}"][aria-selected="true"][aria-busy="false"]')`,
    );
    report.scenarios.end = await evaluate(
      `({selected:document.querySelector('.game-row[aria-selected="true"]').textContent,mounted:document.querySelectorAll('.game-row').length})`,
    );
    const clickedIndex = await evaluate(
      `(()=>{const row=document.querySelector('.game-row[aria-busy="false"]');row.querySelector('.game-title').click();return Number(row.getAttribute('aria-rowindex'));})()`,
    );
    await until(
      `document.querySelector('.game-row[aria-rowindex="${clickedIndex}"][aria-selected="true"][aria-busy="false"]')`,
    );
    report.scenarios.mouseSelection = await evaluate(
      `document.querySelector('.game-row[aria-selected="true"]').id`,
    );
    await key("ArrowUp", 38);
    await until(
      `document.querySelector('.game-row[aria-rowindex="${Math.max(2, clickedIndex - 1)}"][aria-selected="true"][aria-busy="false"]')`,
    );
    const selected = await evaluate(
      `document.querySelector('.game-row[aria-selected="true"]')?.id`,
    );
    const before = await evaluate(
      `document.querySelector('.library-scroll').scrollTop`,
    );
    await click("Mods");
    await until(`!document.querySelector('.library-panel')`);
    await click("Library");
    await until(loaded);
    report.scenarios.return = await evaluate(
      `({scrollTop:document.querySelector('.library-scroll').scrollTop,selected:document.querySelector('.game-row[aria-selected="true"]')?.id})`,
    );
    assert.equal(report.scenarios.return.selected, selected);
    assert.equal(report.scenarios.return.scrollTop, before);
    await command("Emulation.setDeviceMetricsOverride", {
      width: 900,
      height: 650,
      deviceScaleFactor: 1.25,
      mobile: false,
    });
    await sleep(200);
    await until(loaded);
    report.scenarios.emulatedResize = await evaluate(
      `({viewport:[innerWidth,innerHeight],dpr:devicePixelRatio,mounted:document.querySelectorAll('.game-row').length})`,
    );
    await command("Emulation.clearDeviceMetricsOverride");
    await evaluate(`document.querySelector('.diagnostics').open=true`);
    await sleep(1100);
    report.final = await snapshot();
    assert(report.scenarios.rapid.maxActiveRequests <= 1);
    assert(report.scenarios.rapid.maxCachedPages <= 6);
    assert(report.scenarios.rapid.maxMountedRows <= 40);
  }
} catch (error) {
  report.failure = error.message;
  throw error;
} finally {
  socket.close();
  if (output)
    await writeFile(output, JSON.stringify(report, null, 2), { flag: "wx" });
  console.log(
    JSON.stringify({
      total: report.initial?.total,
      firstUsableMs: report.initial?.firstUsableMs,
      maxRows: report.scenarios.rapid?.maxMountedRows,
      maxPages: report.scenarios.rapid?.maxCachedPages,
      error: report.error,
      failure: report.failure,
    }),
  );
}
