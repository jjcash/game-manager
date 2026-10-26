// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { connectWebview } from "./webview.mjs";
const [endpoint = "http://127.0.0.1:9223", output] = process.argv.slice(2);
const profile = process.argv.includes("--profile");
const cycles = Number(
  process.argv.find((arg) => arg.startsWith("--cycles="))?.split("=")[1] ?? 3,
);
assert(Number.isInteger(cycles) && cycles >= 1 && cycles <= 50);
const { evaluate, until, snapshot, command, socket, sleep, key } =
  await connectWebview(endpoint);
const ready = `document.querySelector('.library-panel[data-query-ready="true"]')`;
const visible = `Array.from(document.querySelectorAll('.game-row')).filter(e=>{const a=e.getBoundingClientRect(),b=document.querySelector('.library-scroll').getBoundingClientRect();return a.bottom>b.top&&a.top<b.bottom})`;
const artworkReady = `(()=>{const rows=${visible};return rows.length>0&&rows.every(e=>{const a=e.querySelector('.thumbnail'),img=a?.querySelector('img');return a&&a.dataset.artworkStatus!=='loading'&&(!img||(img.complete&&img.naturalWidth===64&&img.naturalHeight===36))})})()`;
const nativeStats = () =>
  evaluate(`window.__TAURI_INTERNALS__.invoke('artwork_get_stats')`);
const report = {
  scenarios: [],
  timestamp: new Date().toISOString(),
  cacheState: "Derived cache as selected by runner; OS caches retained",
};
async function marker(label) {
  await evaluate(
    `window.__gmArtwork.actions.push({label:${JSON.stringify(label)},at:performance.now(),loadingImages:document.querySelectorAll('.thumbnail[data-artwork-status="loading"]').length})`,
  );
}
async function record(name) {
  await until(ready);
  await until(artworkReady);
  await sleep(1050);
  const metrics = await snapshot();
  const native = await nativeStats();
  assert(metrics.maxMountedRows <= 40);
  assert(metrics.maxCachedPages <= 6);
  assert(metrics.maxActiveRequests <= 1);
  assert(metrics.artwork.maxActive <= 2);
  assert(metrics.artwork.maxQueued <= 32);
  assert(metrics.artwork.cached <= 128);
  assert(native.maxActive <= 2);
  assert(native.memoryEntries <= 128);
  assert(native.diskEntries <= 256);
  assert(native.diskBytes <= 4194304);
  report.scenarios.push({
    name,
    at: await evaluate(`performance.now()`),
    metrics,
    native,
    visible: await evaluate(
      `${visible}.map(e=>({id:e.id,status:e.querySelector('.thumbnail').dataset.artworkStatus}))`,
    ),
  });
}
try {
  if (profile) {
    await command("Profiler.enable");
    await command("Profiler.setSamplingInterval", { interval: 1000 });
    await command("Profiler.start");
    report.profileBrowserStart = await evaluate(`performance.now()`);
  }
  report.timeOrigin = await evaluate(`performance.timeOrigin`);
  await until(ready);
  await until(artworkReady);
  await evaluate(`document.querySelector('.diagnostics').open=true`);
  await sleep(1050);
  report.initial = await snapshot();
  report.visibleThumbnailsReadyMs = await evaluate(
    `Math.max(...${visible}.map(e=>Number(e.querySelector('.thumbnail').dataset.artworkReadyAt)||0))-${report.initial.startedAt}`,
  );
  assert(
    await evaluate(`document.querySelectorAll('.thumbnail img').length>0`),
    "Real local thumbnails must be displayed",
  );
  if (output) {
    const screenshot = await command("Page.captureScreenshot", {
      format: "png",
    });
    await writeFile(`${output}.png`, Buffer.from(screenshot.data, "base64"));
  }
  await evaluate(
    `(()=>{const m=window.__gmArtwork={frames:[],spikes:[],longTasks:[],actions:[],over50:0,maxRows:0,running:true};let previous=0;const tick=now=>{if(!m.running)return;if(previous&&document.visibilityState==='visible'){const d=now-previous;m.frames.push(d);if(m.frames.length>4096)m.frames.shift();if(d>50){m.over50++;m.spikes.push({start:previous,duration:d});if(m.spikes.length>128)m.spikes.shift();}}previous=now;m.maxRows=Math.max(m.maxRows,document.querySelectorAll('.game-row').length);m.raf=requestAnimationFrame(tick)};m.raf=requestAnimationFrame(tick);try{m.observer=new PerformanceObserver(list=>{for(const e of list.getEntries()){m.longTasks.push({start:e.startTime,duration:e.duration});if(m.longTasks.length>128)m.longTasks.shift();}});m.observer.observe({type:'longtask',buffered:false});m.longTasksSupported=true;}catch{m.longTasksSupported=false;}})()`,
  );
  await record("initial");
  await marker("slowScroll");
  await evaluate(
    `(async()=>{const e=document.querySelector('.library-scroll');for(let i=0;i<120;i++){e.scrollTop+=8;await new Promise(requestAnimationFrame)}})()`,
  );
  await record("slowScroll");
  await marker("immediateWarmReturn");
  await evaluate(`document.querySelector('.library-scroll').scrollTop=0`);
  await record("immediateWarmReturn");
  const total = report.initial.total;
  await marker("rapidDistantScroll");
  await evaluate(
    `(async()=>{const e=document.querySelector('.library-scroll');for(let i=0;i<100;i++){e.scrollTop=((i*7919)%${total})*38;await new Promise(requestAnimationFrame)}})()`,
  );
  await record("rapidDistantScroll");
  for (let cycle = 0; cycle < cycles; cycle++) {
    await marker(`distributedJumps-cycle-${cycle}`);
    const start = await evaluate(`performance.now()`);
    for (let i = 0; i < 20; i++) {
      const index = Math.floor((i * (total - 1)) / 19);
      await evaluate(
        `document.querySelector('.library-scroll').scrollTop=${index * 38}`,
      );
      await until(ready);
      await until(
        `document.querySelector('.game-row[aria-rowindex="${index + 2}"][aria-busy="false"]')`,
      );
      await until(artworkReady);
    }
    await record(`distributedJumps-cycle-${cycle}`);
    report.scenarios.at(-1).durationMs = await evaluate(
      `performance.now()-${start}`,
    );
  }
  await marker("selectionAndReturn");
  await evaluate(`document.querySelector('[role="grid"]').focus()`);
  await key("End", 35);
  await until(
    `document.querySelector('.game-row[aria-rowindex="${total + 1}"][aria-selected="true"][aria-busy="false"]')`,
  );
  await record("loadedEndSelection");
  report.browserHeap = await command("Runtime.getHeapUsage");
  report.invalidIdentifierResults = await evaluate(
    `Promise.all(['../secret','C:\\\\Windows\\\\secret.png','file:///secret','syn-000000000000002a-g-99999'].map(async id=>({id,result:await window.__TAURI_INTERNALS__.invoke('artwork_get_thumbnail',{id})})))`,
  );
  assert(
    report.invalidIdentifierResults.every((r) => r.result.status === "invalid"),
  );
  report.frames = await evaluate(
    `(()=>{const m=window.__gmArtwork;m.running=false;cancelAnimationFrame(m.raf);m.observer?.disconnect();return {intervals:m.frames,spikes:m.spikes,longTasks:m.longTasks,actions:m.actions,over50:m.over50,maxRows:m.maxRows,longTasksSupported:m.longTasksSupported}})()`,
  );
  assert(report.frames.maxRows <= 40);
} catch (error) {
  report.failure = error.message;
  throw error;
} finally {
  if (profile) {
    const captured = await command("Profiler.stop");
    if (output)
      await writeFile(`${output}.cpuprofile`, JSON.stringify(captured.profile));
  }
  socket.close();
  if (output)
    await writeFile(output, JSON.stringify(report, null, 2), { flag: "wx" });
  console.log(
    JSON.stringify({
      total: report.initial?.total,
      scenarios: report.scenarios.length,
      failure: report.failure,
    }),
  );
}
