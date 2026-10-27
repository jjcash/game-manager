// SPDX-License-Identifier: GPL-3.0-or-later
import { writeFile } from "node:fs/promises";
import assert from "node:assert/strict";
import { connectWebview } from "./webview.mjs";
const [endpoint = "http://127.0.0.1:9223", output] = process.argv.slice(2);
const {
  evaluate,
  until,
  command,
  snapshot,
  click,
  key,
  socket,
  target,
  sleep,
} = await connectWebview(endpoint);
const report = {
  timestamp: new Date().toISOString(),
  timeOrigin: await evaluate(`performance.timeOrigin`),
  url: target.url,
  scenarios: [],
  deepWindows: [],
};
const base = {
  search: "",
  installed: "all",
  sourceId: null,
  yearFrom: null,
  yearTo: null,
  sort: "title",
  direction: "asc",
};
const ready = `document.querySelector('.library-panel[data-query-ready="true"]')`;
async function input(label, value) {
  await evaluate(
    `(()=>{const e=document.querySelector('[aria-label=${JSON.stringify(label)}]');if(window.__gmQueryBench){window.__gmQueryBench.actions.push({at:performance.now(),label:${JSON.stringify(label)}});if(window.__gmQueryBench.actions.length>128)window.__gmQueryBench.actions.shift();}const proto=e.tagName==='SELECT'?HTMLSelectElement.prototype:HTMLInputElement.prototype;Object.getOwnPropertyDescriptor(proto,'value').set.call(e,${JSON.stringify(String(value))});e.dispatchEvent(new Event(e.tagName==='SELECT'?'change':'input',{bubbles:true}));})()`,
  );
}
async function waitCriteria(criteria) {
  await until(
    `(()=>{const e=document.querySelector('.library-panel');return e?.dataset.queryReady==='true' && Object.entries(${JSON.stringify(criteria)}).every(([k,v])=>JSON.parse(e.dataset.queryKey)[k]===v);})()`,
  );
}
async function record(name, expected) {
  await waitCriteria(expected);
  await sleep(1050);
  const m = await snapshot();
  const count = await evaluate(
    `Number(document.querySelector('.library-panel').dataset.resultCount)`,
  );
  const first = await evaluate(
    `document.querySelector('.game-title')?.textContent??null`,
  );
  if (count > 0)
    assert(
      m.visibleRows > 0,
      "Loaded result rows must intersect the actual virtual viewport",
    );
  assert(
    Number.isFinite(m.criteriaToVisibleMs),
    "Criteria-to-visible commit must be measured",
  );
  assert(m.cachedPages <= 6);
  assert(m.activeRequests <= 1);
  assert(m.maxMountedRows <= 40);
  report.scenarios.push({
    name,
    criteria: expected,
    count,
    first,
    metrics: m,
    browserTimeMs: await evaluate(`performance.now()`),
  });
  return count;
}
async function measureFilteredWindows(name, count) {
  // Observe loaded, visible rows; include automation/polling overhead and keep
  // these DOM-arrival durations separate from criteria commits and SQL timers.
  const samples = [];
  for (let step = 0; step < 20; step++) {
    const index = Math.floor((step * Math.max(0, count - 10)) / 19);
    const started = await evaluate(
      `(()=>{const started=performance.now();document.querySelector('.library-scroll').scrollTop=${index * 38};return started})()`,
    );
    await until(
      `(()=>{const row=document.querySelector('.game-row[aria-rowindex="${index + 2}"][aria-busy="false"]');if(!row)return false;const r=row.getBoundingClientRect(),b=document.querySelector('.library-scroll').getBoundingClientRect();return r.bottom>b.top&&r.top<b.bottom})()`,
    );
    samples.push({
      index,
      observedLoadedMs: (await evaluate("performance.now()")) - started,
    });
  }
  report.deepWindows.push({ name, count, samples, metrics: await snapshot() });
}
try {
  await until(ready);
  await evaluate(`document.querySelector('.diagnostics').open=true`);
  await sleep(1100);
  report.initial = await snapshot();
  const total = report.initial.total;
  // Explicit test-only recorder spans criteria generations, including the debounce
  // and native requests. Keep samples bounded and record JS long tasks separately.
  await evaluate(
    `(()=>{const m=window.__gmQueryBench={frames:[],longTasks:[],spikes:[],actions:[],over50:0,maxRows:0,maxActive:0,maxCached:0,running:true};let previous=0;const tick=now=>{if(!m.running)return;if(previous&&document.visibilityState==='visible'){const d=now-previous;m.frames.push(d);if(m.frames.length>4096)m.frames.shift();if(d>50){m.over50++;m.spikes.push({start:now-d,duration:d});if(m.spikes.length>128)m.spikes.shift();}}previous=now;m.maxRows=Math.max(m.maxRows,document.querySelectorAll('.game-row').length);m.raf=requestAnimationFrame(tick);};m.raf=requestAnimationFrame(tick);try{m.observer=new PerformanceObserver(list=>{for(const e of list.getEntries()){m.longTasks.push({start:e.startTime,duration:e.duration});if(m.longTasks.length>128)m.longTasks.shift();}});m.observer.observe({type:'longtask',buffered:false});}catch{m.longTasksSupported=false;}})()`,
  );
  for (const [name, search] of [
    ["common", "chron"],
    ["rare", "Afterlight 2"],
    ["none", "no-such-title-xyz"],
    ["empty", ""],
    ["unicode", "星の"],
    ["literal", "%_\\\"'"],
  ]) {
    await input("Search titles", search);
    const count = await record(name, { ...base, search });
    if (name === "none" || name === "literal") assert.equal(count, 0);
    if (name === "empty") assert.equal(count, total);
    if (name === "common" && count) {
      await evaluate(`document.querySelector('[role="grid"]').focus()`);
      await key("End", 35);
      await until(
        `document.querySelector('.game-row[aria-rowindex="${count + 1}"][aria-busy="false"][aria-selected="true"]')`,
      );
    }
  }
  await input("Search titles", "");
  await waitCriteria(base);
  await input("Installation filter", "installed");
  const installed = await record("installed", {
    ...base,
    installed: "installed",
  });
  assert(installed > 0 && installed < total);
  await input("Sort games", "title:desc");
  await record("installedDescDeep", {
    ...base,
    installed: "installed",
    direction: "desc",
  });
  await measureFilteredWindows("installedTitleDesc", installed);
  await evaluate(`document.querySelector('[role="grid"]').focus()`);
  await key("End", 35);
  await until(
    `document.querySelector('.game-row[aria-rowindex="${installed + 1}"][aria-busy="false"][aria-selected="true"]')`,
  );
  await input("Sort games", "title:asc");
  await input("Installation filter", "notInstalled");
  const absent = await record("notInstalled", {
    ...base,
    installed: "notInstalled",
  });
  assert.equal(installed + absent, total);
  await measureFilteredWindows("notInstalledTitleAsc", absent);
  await input("Installation filter", "all");
  await waitCriteria(base);
  const sourceId = await evaluate(
    `document.querySelector('[aria-label="Library source filter"] option[value]:not([value=""])').value`,
  );
  await input("Library source filter", sourceId);
  await record("source", { ...base, sourceId });
  await input("Release year from", "2000");
  await input("Release year to", "2001");
  await input("Sort games", "releaseDate:desc");
  await record("narrowRange", {
    ...base,
    sourceId,
    yearFrom: 2000,
    yearTo: 2001,
    sort: "releaseDate",
    direction: "desc",
  });
  await click("Reset all");
  await waitCriteria(base);
  for (const value of ["title:desc", "releaseDate:asc", "releaseDate:desc"]) {
    await input("Sort games", value);
    const [sort, direction] = value.split(":");
    const expected = { ...base, sort, direction };
    await record(value, expected);
    await evaluate(`document.querySelector('[role="grid"]').focus()`);
    await key("End", 35);
    await until(
      `document.querySelector('.game-row[aria-rowindex="${total + 1}"][aria-busy="false"][aria-selected="true"]')`,
    );
    report.scenarios.at(-1).end = await evaluate(
      `document.querySelector('.game-row[aria-selected="true"]').textContent`,
    );
  }
  await click("Reset all");
  await waitCriteria(base);
  for (const term of ["c", "ch", "chr", "chro", "chron"]) {
    await input("Search titles", term);
    await sleep(25);
  }
  await record("rapidEdits", { ...base, search: "chron" });
  await input("Installation filter", "installed");
  await input("Library source filter", sourceId);
  await input("Release year from", "1990");
  await input("Release year to", "2020");
  await input("Sort games", "releaseDate:desc");
  const combined = {
    ...base,
    search: "chron",
    installed: "installed",
    sourceId,
    yearFrom: 1990,
    yearTo: 2020,
    sort: "releaseDate",
    direction: "desc",
  };
  const matches = await record("combined", combined);
  if (matches) {
    await evaluate(`document.querySelector('[role="grid"]').focus()`);
    await key("End", 35);
    await until(
      `document.querySelector('.game-row[aria-rowindex="${matches + 1}"][aria-busy="false"]')`,
    );
  }
  for (let i = 0; i < 16; i++) {
    await input("Installation filter", i % 2 ? "installed" : "notInstalled");
    await sleep(20);
  }
  await record("rapidFilters", combined);
  await click("Reset all");
  await waitCriteria(base);
  // Repeat ordinary long-distance browsing for comparison with M0.3.
  await evaluate(
    `(async()=>{const e=document.querySelector('.library-scroll');for(let i=0;i<100;i++){e.scrollTop=((i*7919)%${total})*38;await new Promise(requestAnimationFrame);}})()`,
  );
  await until(ready);
  await record("baselineScroll", base);
  report.frames = await evaluate(
    `(()=>{const m=window.__gmQueryBench;m.running=false;cancelAnimationFrame(m.raf);m.observer?.disconnect();return {intervals:m.frames,longTasks:m.longTasks,spikes:m.spikes,actions:m.actions,longTasksSupported:m.longTasksSupported!==false,over50:m.over50,maxRows:m.maxRows};})()`,
  );
  assert(report.frames.maxRows <= 40);
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
      scenarios: report.scenarios.length,
      maxRows: report.frames?.maxRows,
      failure: report.failure,
    }),
  );
}
