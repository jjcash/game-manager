// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { writeFile, appendFile } from "node:fs/promises";
import { connectWebview } from "./webview.mjs";
const [endpoint, output, ...flags] = process.argv.slice(2);
const option = (name, fallback) =>
  flags.find((s) => s.startsWith(`--${name}=`))?.split("=")[1] ?? fallback;
const mode = option("case", "idle"),
  iterations = Number(option("iterations", 20)),
  cycles = Number(option("cycles", 1));
const { evaluate, until, snapshot, key, command, socket, sleep, onEvent } =
  await connectWebview(endpoint);
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
const settled = `!document.querySelector('.thumbnail[data-artwork-status="loading"]') && Array.from(document.querySelectorAll('.thumbnail img')).every(img=>img.complete&&img.naturalWidth===64&&img.naturalHeight===36)`;
const report = {
  mode,
  iterations,
  cycles,
  timestamp: new Date().toISOString(),
  diagnosticsOpen: !flags.includes("--closed-diagnostics"),
  cases: [],
  ui: [],
  completion: [],
  cancel: [],
  memoryCycles: [],
};
let tracing = false,
  traceReady,
  traceRemove;
async function marker(label) {
  return evaluate(
    `(()=>{performance.mark(${JSON.stringify("gm:" + label)});const at=performance.now();window.__gmBackground.phase=${JSON.stringify(label)};window.__gmBackground.actions.push({label:${JSON.stringify(label)},at});return at})()`,
  );
}
async function input(label, value) {
  await marker("ui:" + label);
  await evaluate(
    `(()=>{const e=document.querySelector('[aria-label=${JSON.stringify(label)}]');const proto=e.tagName==='SELECT'?HTMLSelectElement.prototype:HTMLInputElement.prototype;Object.getOwnPropertyDescriptor(proto,'value').set.call(e,${JSON.stringify(String(value))});e.dispatchEvent(new Event(e.tagName==='SELECT'?'change':'input',{bubbles:true}));})()`,
  );
}
async function criteria(expected) {
  await until(
    `(()=>{const e=document.querySelector('.library-panel');return e?.dataset.queryReady==='true'&&Object.entries(${JSON.stringify(expected)}).every(([k,v])=>JSON.parse(e.dataset.queryKey)[k]===v)})()`,
  );
}
async function jobDone(id) {
  await until(
    `window.__TAURI_INTERNALS__.invoke('synthetic_list').then(r=>r.jobs.some(j=>j.id===${id}&&['completed','failed','cancelled','interrupted'].includes(j.state)))`,
  );
  return (await invoke("synthetic_list", { detailed: true })).jobs.find(
    (j) => j.id === id,
  );
}
async function cancel(id) {
  const start = await evaluate(`performance.now()`);
  await invoke("synthetic_cancel", { id });
  const final = await jobDone(id);
  report.cancel.push({
    id,
    observedMs: (await evaluate(`performance.now()`)) - start,
    final,
  });
  return final;
}
async function record(name, expected) {
  await criteria(expected);
  await until(settled);
  await sleep(1050);
  const m = await snapshot();
  assert(
    m.maxMountedRows <= 36 && m.maxCachedPages <= 6 && m.maxActiveRequests <= 1,
  );
  assert(m.artwork.maxActive <= 2 && m.artwork.maxQueued <= 32);
  report.ui.push({
    name,
    at: await evaluate("performance.now()"),
    metrics: m,
    jobs: await invoke("synthetic_list", { detailed: true }),
  });
}
async function startTrace() {
  traceReady = new Promise((resolve) => {
    traceRemove = onEvent((m) => {
      if (m.method === "Tracing.tracingComplete") resolve(m.params);
    });
  });
  await command("Tracing.start", {
    transferMode: "ReturnAsStream",
    traceConfig: {
      recordMode: "recordContinuously",
      traceBufferSizeInKb: 16384,
      includedCategories: [
        "toplevel",
        "devtools.timeline",
        "disabled-by-default-devtools.timeline",
        "v8",
        "blink.user_timing",
        "disabled-by-default-v8.gc",
      ],
    },
  });
  tracing = true;
}
async function finishTrace() {
  await command("Tracing.end");
  const completed = await traceReady;
  traceRemove();
  let bytes = 0;
  for (;;) {
    const chunk = await command("IO.read", {
      handle: completed.stream,
      size: 65536,
    });
    const data = Buffer.from(
      chunk.data,
      chunk.base64Encoded ? "base64" : "utf8",
    );
    bytes += data.length;
    if (bytes > 64 * 1024 * 1024) {
      report.traceTruncated = true;
      break;
    }
    await appendFile(`${output}.trace.json`, data);
    if (chunk.eof) break;
  }
  await command("IO.close", { handle: completed.stream });
  report.traceBytes = bytes;
  tracing = false;
}
try {
  if (flags.includes("--trace")) await startTrace();
  await until(ready);
  await until(settled);
  await evaluate(
    `document.querySelector('.diagnostics').open=${!flags.includes("--closed-diagnostics")}`,
  );
  await sleep(1050);
  report.initial = await snapshot();
  report.timeOrigin = await evaluate("performance.timeOrigin");
  report.initialJobs = await invoke("synthetic_list", { detailed: true });
  const info = await invoke("library_get_info", { criteria: base });
  assert.equal(info.syntheticJobs, true);
  report.workers = info.artworkWorkers;
  report.initialVisibleArtworkMs = info.artworkWorkers
    ? await evaluate(
        `(()=>{const box=document.querySelector('.library-scroll').getBoundingClientRect();return Math.max(...Array.from(document.querySelectorAll('.game-row')).filter(e=>{const r=e.getBoundingClientRect();return r.bottom>box.top&&r.top<box.bottom}).map(e=>Number(e.querySelector('.thumbnail').dataset.artworkReadyAt)||${report.initial.startedAt}))-${report.initial.startedAt}})()`,
      )
    : null;
  const total = info.total;
  if (flags.includes("--restart-check")) {
    assert.equal(report.initialJobs.active, 0);
    assert.equal(report.initialJobs.queued, 0);
    assert(
      report.initialJobs.jobs.some(
        (j) => j.state === "cancelled" && j.committed <= j.processed,
      ),
    );
    report.restartCheck = true;
  } else if (flags.includes("--shutdown-job")) {
    report.shutdownJob = await invoke("synthetic_start", {
      request: {
        kind: "mixed",
        records: 100000,
        seed: 20261026,
        intensity: "slow",
        fault: "none",
      },
    });
    await until(
      `window.__TAURI_INTERNALS__.invoke('synthetic_list').then(r=>r.jobs.some(j=>j.id===${report.shutdownJob.id}&&j.state==='running'&&j.committed>=64))`,
    );
    report.beforeShutdown = await invoke("synthetic_list", { detailed: true });
  } else {
    await evaluate(
      `(()=>{const m=window.__gmBackground={frames:[],spikes:[],longTasks:[],actions:[],inputs:[],phases:{},phase:"setup",over50:0,maxRows:0,running:true};let previous=0;const tick=now=>{if(!m.running)return;if(previous&&document.visibilityState==='visible'){const d=now-previous;const p=m.phases[m.phase]??(m.phases[m.phase]={samples:[],total:0,over50:0,max:0});p.samples.push(d);if(p.samples.length>1024)p.samples.shift();p.total++;p.max=Math.max(p.max,d);if(d>50)p.over50++;m.frames.push(d);if(m.frames.length>4096)m.frames.shift();if(d>50){m.over50++;m.spikes.push({start:previous,duration:d});if(m.spikes.length>128)m.spikes.shift();}}previous=now;m.maxRows=Math.max(m.maxRows,document.querySelectorAll('.game-row').length);m.raf=requestAnimationFrame(tick)};m.raf=requestAnimationFrame(tick);m.input=e=>{if(!['INPUT','SELECT'].includes(e.target.tagName))return;const at=performance.now();requestAnimationFrame(now=>{m.inputs.push({at,duration:now-at,type:e.type,label:e.target.getAttribute('aria-label')});if(m.inputs.length>128)m.inputs.shift()})};document.addEventListener('input',m.input,true);document.addEventListener('change',m.input,true);try{m.observer=new PerformanceObserver(list=>{for(const e of list.getEntries()){m.longTasks.push({start:e.startTime,duration:e.duration});if(m.longTasks.length>128)m.longTasks.shift()}});m.observer.observe({type:'longtask',buffered:false});m.longTasksSupported=true}catch{m.longTasksSupported=false}})()`,
    );
    const kind = {
      cpu: "cpu",
      hash: "hash",
      write: "write",
      mixed0: "mixed",
      mixed1: "mixed",
      mixed2: "mixed",
      priority: "mixed",
    }[mode];
    await evaluate(`(()=>{const b=window.__gmLoad={stop:false,artStop:false,artRequests:0,artBusy:0,completed:[],started:[],current:null,failure:null};const invoke=window.__TAURI_INTERNALS__.invoke;
      b.jobs=(async()=>{while(!b.stop&&${JSON.stringify(Boolean(kind))}){try{if(b.started.length>=64)throw new Error('Benchmark job-cycle cap');const job=await invoke('synthetic_start',{request:{kind:${JSON.stringify(kind ?? "cpu")},records:100000,seed:20261026,intensity:'saturated',fault:'none'}});b.current=job.id;b.started.push(job.id);for(;;){const r=await invoke('synthetic_list');const j=r.jobs.find(j=>j.id===job.id);if(['completed','failed','cancelled'].includes(j.state)){b.completed.push(j);break;}await new Promise(r=>setTimeout(r,100));if(b.stop){await invoke('synthetic_cancel',{id:job.id});}}}catch(e){b.failure=String(e);break}}})();
      b.art=Promise.all(Array.from({length:${info.artworkWorkers}},(_,worker)=>(async()=>{let index=worker;while(!b.artStop){const r=await invoke('artwork_get_thumbnail',{id:'syn-000000000000002a-g-'+String(index%${total}).padStart(5,'0')});b.artRequests++;if(r.status==='busy')b.artBusy++;index+=${Math.max(1, info.artworkWorkers)};}})()));})()`);
    const cases = [
      ["default", base],
      ["installed", { ...base, installed: "installed" }],
      ["notInstalled", { ...base, installed: "notInstalled" }],
      [
        "combined",
        {
          ...base,
          search: "chron",
          installed: "installed",
          sourceId: "archive",
          yearFrom: 1990,
          yearTo: 2020,
          sort: "releaseDate",
          direction: "desc",
        },
      ],
    ];
    for (const [name, criterion] of cases) {
      await marker("ipc:" + name);
      const result = await evaluate(
        `(async()=>{const invoke=window.__TAURI_INTERNALS__.invoke,criteria=${JSON.stringify(criterion)};const first=await invoke('library_get_info',{criteria});const request={limit:100,offset:Math.max(0,first.total-100),criteria};const expected=JSON.stringify((await invoke('library_query',{request})).items);const metadata=[],windows=[],counts=[],pages=[],countPool=[],pagePool=[],load=[];for(let i=0;i<${iterations};i++){let at=performance.now();const info=await invoke('library_get_info',{criteria});metadata.push(performance.now()-at);counts.push(info.queryMs);countPool.push(info.poolWaitMs);at=performance.now();const window=await invoke('library_query',{request});windows.push(performance.now()-at);pages.push(window.queryMs);pagePool.push(window.poolWaitMs);if(first.total!==info.total||expected!==JSON.stringify(window.items))throw new Error('Count/window changed under synthetic writes');const jobs=await invoke('synthetic_list');load.push({at:performance.now(),active:jobs.active,queued:jobs.queued,resource:jobs.scheduling.workersActive})}return {name:${JSON.stringify(name)},count:first.total,metadata,windows,counts,pages,countPool,pagePool,load}})()`,
      );
      report.cases.push(result);
    }
    await evaluate(
      `(async()=>{window.__gmLoad.artStop=true;await window.__gmLoad.art})()`,
    );
    await marker("ui:remountUnderLoad");
    await evaluate(
      `Array.from(document.querySelectorAll('.nav-item')).find(b=>b.textContent.trim()==='Mods').click()`,
    );
    await until(`document.querySelector('.placeholder-panel')`);
    await evaluate(
      `Array.from(document.querySelectorAll('.nav-item')).find(b=>b.textContent.trim()==='Library').click()`,
    );
    await criteria(base);
    await until(settled);
    await evaluate(
      `document.querySelector('.diagnostics').open=${!flags.includes("--closed-diagnostics")}`,
    );
    await sleep(1050);
    report.remountUnderLoad = {
      metrics: await snapshot(),
      jobs: await invoke("synthetic_list", { detailed: true }),
    };
    await evaluate(
      `Array.from(document.querySelectorAll('button')).find(b=>b.textContent.trim()==='Refresh library').click()`,
    );
    await criteria(base);
    await until(settled);
    await marker("ui:search");
    await evaluate(
      `document.querySelector('[aria-label="Search titles"]').focus()`,
    );
    await command("Input.insertText", { text: "chron" });
    await record("search", { ...base, search: "chron" });
    await input("Installation filter", "installed");
    await input("Library source filter", "archive");
    await input("Release year from", "1990");
    await input("Release year to", "2020");
    await input("Sort games", "releaseDate:desc");
    await record("combined", {
      ...base,
      search: "chron",
      installed: "installed",
      sourceId: "archive",
      yearFrom: 1990,
      yearTo: 2020,
      sort: "releaseDate",
      direction: "desc",
    });
    await evaluate(
      `Array.from(document.querySelectorAll('button')).find(b=>b.textContent.trim()==='Reset all').click()`,
    );
    await criteria(base);
    await input("Search titles", "");
    await criteria(base);
    await input("Installation filter", "installed");
    await record("installed", { ...base, installed: "installed" });
    await input("Installation filter", "notInstalled");
    await record("notInstalled", { ...base, installed: "notInstalled" });
    if (flags.includes("--trace-sort")) await startTrace();
    await input("Installation filter", "all");
    await input("Sort games", "releaseDate:desc");
    await record("sort", { ...base, sort: "releaseDate", direction: "desc" });
    if (flags.includes("--trace-sort")) await finishTrace();
    await marker("ui:rapidScroll");
    await evaluate(
      `(async()=>{const e=document.querySelector('.library-scroll');for(let i=0;i<60;i++){e.scrollTop=((i*7919)%${total})*38;await new Promise(requestAnimationFrame)}})()`,
    );
    await criteria({ ...base, sort: "releaseDate", direction: "desc" });
    await evaluate(`document.querySelector('[role="grid"]').focus()`);
    await key("End", 35);
    await until(
      `document.querySelector('.game-row[aria-rowindex="${total + 1}"][aria-selected="true"][aria-busy="false"]')`,
    );
    await until(settled);
    if (info.artworkWorkers)
      assert(
        await evaluate(`document.querySelectorAll('.thumbnail img').length>0`),
      );
    await record("loadedEnd", {
      ...base,
      sort: "releaseDate",
      direction: "desc",
    });
    report.load = await evaluate(
      `(async()=>{const b=window.__gmLoad;b.stop=true;if(b.current)await window.__TAURI_INTERNALS__.invoke('synthetic_cancel',{id:b.current});await b.jobs;return {started:b.started,completed:b.completed,artRequests:b.artRequests,artBusy:b.artBusy,failure:b.failure}})()`,
    );
    assert.equal(report.load.failure, null);
    assert(
      report.load.completed.every((j) =>
        ["completed", "cancelled"].includes(j.state),
      ),
    );
    await marker("safety:completion");
    for (const kind of ["cpu", "hash", "write", "mixed"]) {
      const j = await invoke("synthetic_start", {
        request: {
          kind,
          records: 512,
          seed: 20261026,
          intensity: "saturated",
          fault: "none",
        },
      });
      const final = await jobDone(j.id);
      assert.equal(final.state, "completed");
      assert.equal(final.processed, 512);
      assert.equal(
        final.committed,
        ["write", "mixed"].includes(kind) ? 512 : 0,
      );
      report.completion.push(final);
    }
    for (let cycle = 0; cycle < cycles; cycle++) {
      await marker("cycle:" + cycle);
      const j = await invoke("synthetic_start", {
        request: {
          kind: "mixed",
          records: 512,
          seed: 20261026,
          intensity: "gentle",
          fault: "none",
        },
      });
      for (let i = 0; i < 20; i++) {
        const index = Math.floor((i * (total - 1)) / 19);
        await evaluate(
          `document.querySelector('.library-scroll').scrollTop=${index * 38}`,
        );
        await until(
          `document.querySelector('.game-row[aria-rowindex="${index + 2}"][aria-busy="false"]')`,
        );
        await until(settled);
      }
      const final = await jobDone(j.id);
      assert.equal(final.state, "completed");
      report.memoryCycles.push({
        at: await evaluate("performance.now()"),
        job: final,
        heap: await command("Runtime.getHeapUsage"),
      });
    }
    await marker("safety:failure");
    const fail = await invoke("synthetic_start", {
      request: {
        kind: "write",
        records: 128,
        seed: 20261026,
        intensity: "gentle",
        fault: "write",
      },
    });
    report.failureRecovery = await jobDone(fail.id);
    assert.equal(report.failureRecovery.state, "failed");
    assert.equal(report.failureRecovery.committed, 64);
    assert(await evaluate(`!document.querySelector('.library-error')`));
    await marker("safety:cancellation");
    const a = await invoke("synthetic_start", {
      request: {
        kind: "hash",
        records: 100000,
        seed: 20261026,
        intensity: "slow",
        fault: "none",
      },
    });
    await until(
      `window.__TAURI_INTERNALS__.invoke('synthetic_list').then(r=>r.jobs.some(j=>j.id===${a.id}&&j.state==='running'))`,
    );
    const b = await invoke("synthetic_start", {
      request: {
        kind: "cpu",
        records: 1000,
        seed: 20261026,
        intensity: "gentle",
        fault: "none",
      },
    });
    const c = await invoke("synthetic_start", {
      request: {
        kind: "cpu",
        records: 1000,
        seed: 20261026,
        intensity: "gentle",
        fault: "none",
      },
    });
    let rejected = false;
    try {
      await invoke("synthetic_start", {
        request: {
          kind: "cpu",
          records: 1,
          seed: 1,
          intensity: "gentle",
          fault: "none",
        },
      });
    } catch {
      rejected = true;
    }
    assert(rejected);
    const queued = await cancel(b.id);
    assert.equal(queued.state, "cancelled");
    assert.equal(queued.processed, 0);
    await cancel(c.id);
    const active = await cancel(a.id);
    assert.equal(active.state, "cancelled");
    await marker("ui:jobControls");
    const previousMax = Math.max(
      ...(await invoke("synthetic_list")).jobs.map((j) => j.id),
    );
    const selection = await evaluate(
      `document.querySelector('.game-row[aria-selected="true"]')?.id`,
    );
    await evaluate(
      `document.querySelector('[aria-label="Synthetic background testing"]').open=true`,
    );
    await input("Synthetic workload", "hash");
    await input("Synthetic records", "1000");
    await input("Synthetic intensity", "slow");
    await until(
      `Array.from(document.querySelectorAll('button')).some(b=>b.textContent.trim()==='Start synthetic job'&&!b.disabled)`,
    );
    await evaluate(
      `Array.from(document.querySelectorAll('button')).find(b=>b.textContent.trim()==='Start synthetic job').click()`,
    );
    await until(
      `window.__TAURI_INTERNALS__.invoke('synthetic_list').then(r=>r.jobs.some(j=>j.id>${previousMax}&&j.state==='running'))`,
    );
    const control = (await invoke("synthetic_list")).jobs.find(
      (j) => j.id > previousMax,
    );
    await until(
      `document.querySelector('[data-job-id="${control.id}"][data-job-state="running"]')`,
    );
    await evaluate(
      `Array.from(document.querySelectorAll('button')).find(b=>b.textContent.trim()==='Cancel job ${control.id}').click()`,
    );
    const controlled = await jobDone(control.id);
    assert.equal(controlled.state, "cancelled");
    await until(
      `document.querySelector('[data-job-id="${control.id}"][data-job-state="cancelled"]')`,
    );
    assert.equal(
      await evaluate(
        `document.querySelector('.game-row[aria-selected="true"]')?.id`,
      ),
      selection,
    );
    report.nativeControls = { job: controlled, selectionPreserved: true };
    report.finalJobs = await invoke("synthetic_list", { detailed: true });
    assert.equal(report.finalJobs.active, 0);
    assert(
      report.finalJobs.jobs.length <= 8 &&
        report.finalJobs.scheduling.workersMax <= 2,
    );
    assert.equal(report.finalJobs.scheduling.workersActive, 0);
    report.artwork = await invoke("artwork_get_stats");
    assert(
      report.artwork.maxActive <= info.artworkWorkers &&
        report.artwork.memoryEntries <= 128 &&
        report.artwork.diskEntries <= 256,
    );
    report.frames = await evaluate(
      `(()=>{const m=window.__gmBackground;m.running=false;cancelAnimationFrame(m.raf);m.observer?.disconnect();document.removeEventListener('input',m.input,true);document.removeEventListener('change',m.input,true);return {phases:m.phases,intervals:m.frames,spikes:m.spikes,longTasks:m.longTasks,actions:m.actions,inputs:m.inputs,over50:m.over50,maxRows:m.maxRows,longTasksSupported:m.longTasksSupported}})()`,
    );
    assert(report.frames.maxRows <= 36);
    await evaluate(
      `document.querySelector('[aria-label="Synthetic background testing"]').scrollIntoView()`,
    );
    const screenshot = await command("Page.captureScreenshot", {
      format: "png",
    });
    await writeFile(`${output}.png`, Buffer.from(screenshot.data, "base64"));
  }
} catch (error) {
  report.failure = error.message;
  throw error;
} finally {
  if (tracing) await finishTrace();
  socket.close();
  await writeFile(output, JSON.stringify(report, null, 2), { flag: "wx" });
  console.log(
    JSON.stringify({
      mode,
      total: report.initial?.total,
      cases: report.cases.length,
      ui: report.ui.length,
      failure: report.failure,
    }),
  );
}
