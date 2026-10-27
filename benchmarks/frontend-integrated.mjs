// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { writeFile, appendFile } from "node:fs/promises";
import { connectWebview } from "./webview.mjs";
import { observeArrival, watchTarget } from "./arrival.mjs";
import { startIpcRecorder } from "./ipc-recorder.mjs";
const [endpoint, output, ...flags] = process.argv.slice(2);
const option = (name, fallback) =>
  flags.find((s) => s.startsWith(`--${name}=`))?.slice(name.length + 3) ??
  fallback;
const scenario = option("scenario", "deep");
const cycles = Number(option("sustained-cycles", 60));
const recovery = Number(option("recovery-seconds", 60));
const iterations = Number(option("iterations", 30));
const mode = option("case", "idle");
assert(
  [
    "deep",
    "thumbnails",
    "idle",
    "scroll",
    "transitions",
    "artwork",
    "artwork-saturation",
    "complete",
    "cancel",
    "combined",
    "startup",
  ].includes(scenario),
);
assert(Number.isInteger(cycles) && cycles >= 1 && cycles <= 300);
assert(Number.isFinite(recovery) && recovery >= 0 && recovery <= 600);
const { evaluate, until, snapshot, command, socket, sleep, onEvent } =
  await connectWebview(endpoint);
let arrivalObjectId;
let ipcRecorder;
const invoke = (name, args = {}) =>
  evaluate(
    `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(name)},${JSON.stringify(args)})`,
  );
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
const settled = `!document.querySelector('.thumbnail[data-artwork-status="loading"]') && Array.from(document.querySelectorAll('.thumbnail img')).every(i=>i.complete)`;
const report = {
  scenario,
  mode,
  cycles,
  recovery,
  iterations,
  timestamp: new Date().toISOString(),
  probes: [],
  windows: [],
  samples: [],
  arrivalTransport: "stable-callFunctionOn",
  windowPattern: ["combined", "artwork-saturation"].includes(scenario)
    ? "varying-stride-7919"
    : ["scroll", "artwork"].includes(scenario)
      ? "rotating-fixed-20"
      : ["deep", "thumbnails"].includes(scenario)
        ? "distributed-40-stride-17"
        : null,
};
async function input(label, value) {
  await evaluate(
    `(()=>{const e=document.querySelector('[aria-label=${JSON.stringify(label)}]');const proto=e.tagName==='SELECT'?HTMLSelectElement.prototype:HTMLInputElement.prototype;Object.getOwnPropertyDescriptor(proto,'value').set.call(e,${JSON.stringify(String(value))});e.dispatchEvent(new Event(e.tagName==='SELECT'?'change':'input',{bubbles:true}));})()`,
  );
}
async function criteria(c) {
  await until(
    `(()=>{const p=document.querySelector('.library-panel');return p?.dataset.queryReady==='true'&&p.dataset.queryKey===${JSON.stringify(JSON.stringify(c))}})()`,
  );
}
async function arrival(index, c, coarseControl = false) {
  const result = await command("Runtime.callFunctionOn", {
    objectId: arrivalObjectId,
    functionDeclaration: flags.includes("--capture-ipc")
      ? "function(index, queryKey, coarse) { const target=this.__gmWatchTarget(index,queryKey);return Promise.all([this.__gmArrival(index,queryKey,()=>{document.querySelector('.library-scroll').scrollTop=index*38},10000,coarse),target.promise]).then(([r,t])=>({...r,targetDomMs:Math.max(0,t.at-r.startedAt),targetId:t.id})).finally(target.cancel); }"
      : "function(index, queryKey, coarse) { return this.__gmArrival(index, queryKey, () => { document.querySelector('.library-scroll').scrollTop = index * 38; }, 10000, coarse); }",
    arguments: [
      { value: index },
      { value: JSON.stringify(c) },
      { value: coarseControl },
    ],
    returnByValue: true,
    awaitPromise: true,
  });
  if (result.exceptionDetails)
    throw new Error(JSON.stringify(result.exceptionDetails));
  return result.result.value;
}
async function sample(phase, cycle = null) {
  const m = await snapshot();
  assert(
    m.maxMountedRows <= 36 && m.maxCachedPages <= 6 && m.maxActiveRequests <= 1,
  );
  assert(
    m.artwork.maxActive <= 2 &&
      m.artwork.maxQueued <= 32 &&
      m.artwork.cached <= 128,
  );
  const art = await invoke("artwork_get_stats");
  assert(
    art.memoryEntries <= 128 &&
      art.diskEntries <= 256 &&
      art.diskBytes <= 4194304 &&
      art.maxActiveDecodes <= 2 &&
      art.maxActive <= report.workers,
  );
  const jobs = report.syntheticJobs ? await invoke("synthetic_list") : null;
  if (jobs)
    assert(
      jobs.jobs.length <= 8 &&
        jobs.maxActive <= 1 &&
        jobs.queued <= 2 &&
        jobs.scheduling.workersMax <= 2,
    );
  const s = {
    phase,
    cycle,
    at: await evaluate("performance.now()"),
    utc: new Date().toISOString(),
    heap: await command("Runtime.getHeapUsage"),
    dom: await command("Memory.getDOMCounters"),
    artwork: art,
    jobs,
    metrics: m,
  };
  report.samples.push(s);
  // Partial evidence survives a later failed assertion or interrupted session.
  await appendFile(`${output}.samples.jsonl`, JSON.stringify(s) + "\n");
}
async function start(kind, records = 4096, intensity = "gentle") {
  return invoke("synthetic_start", {
    request: { kind, records, seed: 20261026, intensity, fault: "none" },
  });
}
async function finish(id, cancel = false) {
  if (cancel) await invoke("synthetic_cancel", { id });
  await until(
    `window.__TAURI_INTERNALS__.invoke('synthetic_list').then(r=>r.jobs.some(j=>j.id===${id}&&['completed','cancelled','failed'].includes(j.state)))`,
  );
  const job = (await invoke("synthetic_list")).jobs.find((j) => j.id === id);
  assert.equal(job.state, cancel ? "cancelled" : "completed");
  assert(job.committed <= job.processed);
  if (!cancel) {
    assert.equal(job.processed, job.request.records);
    assert.equal(
      job.committed,
      ["write", "mixed"].includes(job.request.kind) ? job.request.records : 0,
    );
  }
  return job;
}
let load;
const loadKind = {
  cpu: "cpu",
  hash: "hash",
  write: "write",
  mixed0: "mixed",
  mixed1: "mixed",
  mixed2: "mixed",
  priority: "mixed",
}[mode];
async function ensureLoad() {
  if (!loadKind) return;
  const j =
    load && (await invoke("synthetic_list")).jobs.find((j) => j.id === load.id);
  if (!j || j.state === "completed")
    load = await start(loadKind, 100000, "saturated");
  else assert(["queued", "running", "finalizing"].includes(j.state));
}
try {
  await until(ready);
  await until(settled);
  await sleep(1100);
  report.initial = await snapshot();
  report.timeOrigin = await evaluate("performance.timeOrigin");
  report.startup = await evaluate(
    `(()=>{const m=JSON.parse(document.querySelector('[aria-label="Performance measurements"]').textContent);const b=document.querySelector('.library-scroll').getBoundingClientRect();return {shellCommit:performance.getEntriesByName('gm:shell-commit').map(e=>e.startTime),libraryInit:m.startedAt,firstUsable:m.startedAt+m.firstUsableMs,visibleThumbnailReady:Math.max(0,...Array.from(document.querySelectorAll('.game-row')).filter(e=>{const r=e.getBoundingClientRect(),i=e.querySelector('.thumbnail img');return r.bottom>b.top&&r.top<b.bottom&&i?.complete&&i.naturalWidth>0}).map(e=>Number(e.querySelector('.thumbnail')?.dataset.artworkReadyAt)||0)),navigation:performance.getEntriesByType('navigation').map(e=>e.toJSON()),viewport:[innerWidth,innerHeight,devicePixelRatio]}})()`,
  );
  // Fix subsequent workload viewport; startup above uses the actual native window.
  await command("Emulation.setDeviceMetricsOverride", {
    width: 1120,
    height: 760,
    deviceScaleFactor: 1,
    mobile: false,
  });
  await until(ready);
  await until(settled);
  const info = await invoke("library_get_info", { criteria: base });
  if (scenario !== "startup") assert.equal(info.syntheticJobs, true);
  report.syntheticJobs = info.syntheticJobs;
  report.workers = info.artworkWorkers;
  const total = info.total;
  await evaluate(
    `window.__gmIntegrated={frames:[],longTasks:[],frameCount:0,over50:0,maxFrame:0,longTaskCount:0,running:${scenario !== "idle"}};(()=>{const m=window.__gmIntegrated;let previous;const tick=t=>{if(!m.running)return;if(previous&&document.visibilityState==='visible'){const d=t-previous;m.frameCount++;m.maxFrame=Math.max(m.maxFrame,d);if(d>50)m.over50++;m.frames.push(d);if(m.frames.length>4096)m.frames.shift()}previous=t;m.raf=requestAnimationFrame(tick)};m.raf=requestAnimationFrame(tick);m.observer=new PerformanceObserver(l=>{for(const e of l.getEntries()){m.longTaskCount++;m.longTasks.push({start:e.startTime,duration:e.duration});if(m.longTasks.length>256)m.longTasks.shift()}});m.observer.observe({type:'longtask'})})()`,
  );
  await sample("before");
  await evaluate(
    `(()=>{window.__gmArrival=${observeArrival.toString()};return true})()`,
  );
  if (flags.includes("--capture-ipc"))
    await evaluate(
      `(()=>{window.__gmWatchTarget=${watchTarget.toString()};return true})()`,
    );
  arrivalObjectId = (
    await command("Runtime.evaluate", {
      expression: "globalThis",
      returnByValue: false,
    })
  ).result.objectId;
  assert(arrivalObjectId, "Expected the synthetic page's global object handle");
  if (flags.includes("--capture-ipc"))
    ipcRecorder = await startIpcRecorder({
      command,
      onEvent,
      timeOrigin: report.timeOrigin,
    });
  if (["deep", "thumbnails"].includes(scenario)) await ensureLoad();
  if (scenario === "deep") {
    for (const installed of ["all", "installed", "notInstalled"]) {
      const c = { ...base, installed };
      const count = (await invoke("library_get_info", { criteria: c })).total;
      for (let round = 0; round < iterations; round++) {
        await ensureLoad();
        // Reverse first/tail order every round; keep samples paired, not sums of percentiles.
        const offsets =
          round % 2
            ? [Math.max(0, count - 100), 0]
            : [0, Math.max(0, count - 100)];
        for (const offset of offsets) {
          const row = await evaluate(
            `(async()=>{const invoke=window.__TAURI_INTERNALS__.invoke;const t=performance.now();const info=await invoke('library_get_info',{criteria:${JSON.stringify(c)}});const u=performance.now();const page=await invoke('library_query',{request:{criteria:${JSON.stringify(c)},limit:100,offset:${offset}}});return {metadataIpcMs:u-t,pageIpcMs:performance.now()-u,countServiceMs:info.queryMs,pageServiceMs:page.queryMs,countPoolMs:info.poolWaitMs,pagePoolMs:page.poolWaitMs,count:info.total,ids:page.items.map(i=>i.id)}})()`,
          );
          assert.equal(row.count, count);
          assert.equal(row.ids.length, Math.min(100, count));
          assert.equal(new Set(row.ids).size, row.ids.length);
          report.probes.push({
            installed,
            offset,
            round,
            ...row,
            jobs: await invoke("synthetic_list"),
          });
        }
      }
      await input("Installation filter", installed);
      await criteria(c);
      for (let step = 0; step < 40; step++) {
        await ensureLoad();
        const index = Math.floor(
          (((step * 17) % 40) * Math.max(0, count - 10)) / 39,
        );
        const r = await arrival(index, c, true);
        const page = await invoke("library_query", {
          request: { criteria: c, limit: 1, offset: index },
        });
        assert(
          r.id === `game-${page.items[0].id}`,
          "Observed canonical row matches native offset",
        );
        if (r.targetId)
          assert.equal(
            r.targetId,
            r.id,
            "Early target and settled window agree on canonical ID",
          );
        report.windows.push({
          installed,
          ...r,
          jobs: await invoke("synthetic_list"),
        });
      }
    }
  } else if (scenario === "thumbnails") {
    for (let step = 0; step < 40; step++) {
      await ensureLoad();
      const index = Math.floor((((step * 17) % 40) * (total - 10)) / 39);
      const row = await arrival(index, base);
      await until(settled);
      const thumbnails = await evaluate(
        `(()=>{const b=document.querySelector('.library-scroll').getBoundingClientRect();const rows=Array.from(document.querySelectorAll('.game-row')).filter(e=>{const r=e.getBoundingClientRect();return r.bottom>b.top&&r.top<b.bottom});return rows.map(e=>({status:e.querySelector('.thumbnail').dataset.artworkStatus,image:Boolean(e.querySelector('.thumbnail img')?.naturalWidth),readyAt:Number(e.querySelector('.thumbnail').dataset.artworkReadyAt)||null}))})()`,
      );
      const readyTimes = thumbnails
        .filter((t) => t.image)
        .map((t) => t.readyAt)
        .filter(Number.isFinite);
      report.windows.push({
        ...row,
        thumbnails,
        thumbnailMs: readyTimes.length
          ? Math.max(0, Math.max(...readyTimes) - row.startedAt)
          : null,
        jobs: await invoke("synthetic_list"),
      });
    }
  } else if (scenario !== "startup") {
    for (let cycle = 0; cycle < cycles; cycle++) {
      const started = Date.now();
      let job;
      if (["complete", "combined"].includes(scenario))
        job = await start("mixed");
      if (scenario === "cancel") {
        job = await start("hash", 100000, "slow");
        await until(
          `window.__TAURI_INTERNALS__.invoke('synthetic_list').then(r=>r.jobs.some(j=>j.id===${job.id}&&j.processed>0))`,
        );
        await finish(job.id, true);
      }
      if (
        ["scroll", "artwork", "artwork-saturation", "combined"].includes(
          scenario,
        )
      ) {
        for (let step = 0; step < 20; step++) {
          const index = ["combined", "artwork-saturation"].includes(scenario)
            ? ((cycle * 20 + step) * 7919) % (total - 10)
            : Math.floor((((step * 7 + cycle) % 20) * (total - 10)) / 19);
          await arrival(index, base);
          if (scenario !== "scroll") await until(settled);
        }
      }
      if (scenario === "transitions") {
        for (const [label, value, expected] of [
          ["Search titles", "chron", { ...base, search: "chron" }],
          [
            "Installation filter",
            "notInstalled",
            { ...base, search: "chron", installed: "notInstalled" },
          ],
          [
            "Sort games",
            "releaseDate:desc",
            {
              ...base,
              search: "chron",
              installed: "notInstalled",
              sort: "releaseDate",
              direction: "desc",
            },
          ],
        ]) {
          await input(label, value);
          await criteria(expected);
        }
        await evaluate(
          `Array.from(document.querySelectorAll('button')).find(b=>b.textContent.trim()==='Reset all').click()`,
        );
        await criteria(base);
      }
      if (["complete", "combined"].includes(scenario)) await finish(job.id);
      // Identical minimum cycle length, including workload and telemetry overhead.
      await sleep(Math.max(0, 1000 - (Date.now() - started)));
      await sample("workload", cycle);
    }
  }
  if (["combined", "artwork-saturation"].includes(scenario)) {
    const art = await invoke("artwork_get_stats");
    assert(
      art.evictions > 0 && art.memoryEntries === 128 && art.diskEntries === 256,
      "The saturation workload must actually exercise native cache eviction",
    );
  }
  if (load) {
    const j = (await invoke("synthetic_list")).jobs.find(
      (j) => j.id === load.id,
    );
    if (j.state !== "completed") await finish(load.id, true);
    load = null;
  }
  await sample("after-workload");
  for (let elapsed = 0; elapsed < recovery; elapsed += 5) {
    await sleep(Math.min(5, recovery - elapsed) * 1000);
    await sample("recovery");
  }
  if (report.syntheticJobs) {
    report.finalJobs = await invoke("synthetic_list", { detailed: true });
    assert.equal(report.finalJobs.active, 0);
    assert.equal(report.finalJobs.queued, 0);
  }
  report.frames = await evaluate(
    `(()=>{const m=window.__gmIntegrated;m.running=false;cancelAnimationFrame(m.raf);m.observer.disconnect();return {intervals:m.frames,frameCount:m.frameCount,over50:m.over50,maxFrame:m.maxFrame,longTaskCount:m.longTaskCount,longTasks:m.longTasks}})()`,
  );
  if (ipcRecorder) {
    report.ipcTimeline = await ipcRecorder.stop();
    ipcRecorder = null;
    assert(
      report.ipcTimeline.length > 0,
      "Expected observed local IPC protocol requests",
    );
  }
} catch (error) {
  report.failure = error.message;
  throw error;
} finally {
  if (load) await invoke("synthetic_cancel", { id: load.id }).catch(() => {});
  if (ipcRecorder)
    report.ipcTimeline = await ipcRecorder.stop().catch((error) => {
      report.ipcCaptureError = error.message;
      return [];
    });
  if (arrivalObjectId)
    await command("Runtime.releaseObject", { objectId: arrivalObjectId }).catch(
      () => {},
    );
  socket.close();
  await writeFile(output, JSON.stringify(report, null, 2), { flag: "wx" });
  console.log(
    JSON.stringify({
      scenario,
      mode,
      samples: report.samples.length,
      probes: report.probes.length,
      windows: report.windows.length,
      failure: report.failure,
    }),
  );
}
