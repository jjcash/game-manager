// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { connectWebview } from "./webview.mjs";
import { traceSummary as summarizeTrace } from "./summarize-layout.mjs";

const [endpoint, output, ...flags] = process.argv.slice(2);
const option = (name, fallback) =>
  flags.find((s) => s.startsWith(`--${name}=`))?.slice(name.length + 3) ??
  fallback;
const repetitions = Number(option("repetitions", 6));
assert(Number.isInteger(repetitions) && repetitions >= 1 && repetitions <= 30);
// Temporary CDP overrides are experiments, never application defaults.
const experiments = {
  baseline: "",
  blockMain: ".main-content { display: block; }",
  gridStart: ".app-shell { align-items: start; }",
  minHeight: ".main-content, .library-panel { min-height: 0; }",
  fixedControls: ".library-controls select { width: 190px; }",
  blockControls: ".library-controls { display: block; }",
  stableScrollbar:
    "html { overflow-y: scroll; } .library-scroll { overflow-y: scroll; }",
  fixedPanel: ".library-panel { flex-shrink: 0; }",
};
const variants = option("variants", "baseline").split(",");
variants.forEach((v) => assert(Object.hasOwn(experiments, v), v));
const traced = !flags.includes("--untraced");
const { evaluate, until, snapshot, command, onEvent, key, socket, sleep } =
  await connectWebview(endpoint);
const base = {
  search: "",
  installed: "all",
  sourceId: null,
  yearFrom: null,
  yearTo: null,
  sort: "title",
  direction: "asc",
};
const sorted = { ...base, sort: "releaseDate", direction: "desc" };
const invoke = (name, args = {}) =>
  evaluate(
    `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(name)},${JSON.stringify(args)})`,
  );
const ready = `document.querySelector('.library-panel[data-query-ready="true"]')`;
const settled = `!document.querySelector('.thumbnail[data-artwork-status="loading"]') && Array.from(document.querySelectorAll('.thumbnail img')).every(i=>i.complete)`;
async function input(label, value) {
  await evaluate(
    `(()=>{const e=document.querySelector('[aria-label=${JSON.stringify(label)}]');e.focus({preventScroll:true});const proto=e.tagName==='SELECT'?HTMLSelectElement.prototype:HTMLInputElement.prototype;Object.getOwnPropertyDescriptor(proto,'value').set.call(e,${JSON.stringify(value)});e.dispatchEvent(new Event(e.tagName==='SELECT'?'change':'input',{bubbles:true}));})()`,
  );
}
async function criteria(c) {
  await until(
    `(()=>{const e=document.querySelector('.library-panel');return e?.dataset.queryReady==='true'&&e.dataset.queryKey===${JSON.stringify(JSON.stringify(c))}&&document.querySelector('.game-row[aria-busy="false"]')})()`,
  );
}
async function startTrace() {
  let remove;
  const complete = new Promise((resolve) => {
    remove = onEvent((m) => {
      if (m.method === "Tracing.tracingComplete") resolve(m.params);
    });
  });
  await command("Tracing.start", {
    transferMode: "ReturnAsStream",
    traceConfig: {
      recordMode: "recordUntilFull",
      traceBufferSizeInKb: 32768,
      includedCategories: [
        "toplevel",
        "devtools.timeline",
        "disabled-by-default-devtools.timeline",
        "disabled-by-default-devtools.timeline.invalidationTracking",
        "blink.user_timing",
        "v8",
      ],
    },
  });
  return async (path) => {
    await command("Tracing.end");
    const done = await complete;
    remove();
    const chunks = [];
    let bytes = 0;
    for (;;) {
      const chunk = await command("IO.read", {
        handle: done.stream,
        size: 65536,
      });
      const b = Buffer.from(
        chunk.data,
        chunk.base64Encoded ? "base64" : "utf8",
      );
      bytes += b.length;
      assert(bytes <= 64 * 1024 * 1024, "Trace exceeds cap; shorten capture");
      chunks.push(b);
      if (chunk.eof) break;
    }
    await command("IO.close", { handle: done.stream });
    const buffer = Buffer.concat(chunks);
    await writeFile(path, buffer, { flag: "wx" });
    const trace = JSON.parse(buffer);
    assert(!trace.metadata?.trace_buffer_overflowed, "Trace buffer overflow");
    return { bytes, summary: summarizeTrace(trace.traceEvents) };
  };
}
const report = {
  timestamp: new Date().toISOString(),
  traced,
  repetitions,
  variants,
  trials: [],
};
try {
  await command("Emulation.setDeviceMetricsOverride", {
    width: 1120,
    height: 760,
    deviceScaleFactor: 1,
    mobile: false,
  });
  await command("Performance.enable");
  await command("DOM.enable");
  await command("CSS.enable");
  await until(ready);
  await until(settled);
  await evaluate(
    `document.querySelector('.diagnostics').open=${!flags.includes("--closed-diagnostics")}`,
  );
  report.info = await invoke("library_get_info", { criteria: base });
  report.browser = await command("Browser.getVersion");
  report.viewport = await evaluate(
    `({width:innerWidth,height:innerHeight,dpr:devicePixelRatio})`,
  );
  const tree = await command("Page.getFrameTree");
  const sheet = await command("CSS.createStyleSheet", {
    frameId: tree.frameTree.frame.id,
  });
  const expected = await invoke("library_query", {
    request: { offset: 0, limit: 100, criteria: sorted },
  });
  assert(expected.items.length === 100);
  for (let repeat = 0; repeat < repetitions; repeat++) {
    // Reverse order each repetition to reduce cache/order bias.
    for (const variant of repeat % 2 ? [...variants].reverse() : variants) {
      await command("CSS.setStyleSheetText", {
        styleSheetId: sheet.styleSheetId,
        text: experiments[variant],
      });
      await input("Sort games", "title:asc");
      const installed = repeat % 2 ? "installed" : "notInstalled";
      await input("Installation filter", installed);
      await criteria({ ...base, installed });
      await until(settled);
      // Select a deep row first: the transition must clear selection and reset scroll.
      await evaluate(
        `document.querySelector('[role="grid"]').focus({preventScroll:true})`,
      );
      await key("End", 35);
      await until(
        `document.querySelector('.game-row[aria-selected="true"][aria-busy="false"]')`,
      );
      await sleep(150);
      const finish = traced ? await startTrace() : null;
      const perfBefore = await command("Performance.getMetrics");
      await evaluate(
        `(()=>{const m=window.__gmLayout={frames:[],longTasks:[],running:true,maxRows:0,commits:[],started:performance.now()};performance.mark('gm:layout:start');let previous=0;const tick=now=>{if(!m.running)return;if(previous&&document.visibilityState==='visible')m.frames.push(now-previous);previous=now;m.maxRows=Math.max(m.maxRows,document.querySelectorAll('.game-row').length);m.raf=requestAnimationFrame(tick)};m.raf=requestAnimationFrame(tick);m.observer=new PerformanceObserver(list=>{for(const e of list.getEntries())m.longTasks.push({start:e.startTime,duration:e.duration})});m.observer.observe({type:'longtask'});m.mutations=new MutationObserver(()=>{const e=document.querySelector('.library-panel');if(e?.dataset.queryReady==='true')m.commits.push({at:performance.now(),criteria:e.dataset.queryKey})});m.mutations.observe(document.querySelector('.library-panel'),{subtree:true,childList:true,attributes:true,attributeFilter:['data-query-ready']});})()`,
      );
      await input("Installation filter", "all");
      await input("Sort games", "releaseDate:desc");
      await criteria(sorted);
      await until(settled);
      await sleep(1100);
      const transition = await evaluate(
        `(()=>{const m=window.__gmLayout;return {frames:[...m.frames],longTasks:[...m.longTasks],commits:[...m.commits],started:m.started,maxRows:m.maxRows,scrollTop:document.querySelector('.library-scroll').scrollTop,selected:document.querySelectorAll('.game-row[aria-selected="true"]').length,focus:document.activeElement?.getAttribute('aria-label'),rows:Array.from(document.querySelectorAll('.game-row')).map(e=>({id:e.id,index:Number(e.getAttribute('aria-rowindex'))-2,top:e.getBoundingClientRect().top,height:e.getBoundingClientRect().height,thumbnail:[e.querySelector('.thumbnail').getBoundingClientRect().width,e.querySelector('.thumbnail').getBoundingClientRect().height]}))}})()`,
      );
      assert.equal(transition.selected, 0);
      assert.equal(transition.scrollTop, 0);
      assert.equal(transition.focus, "Sort games");
      for (const row of transition.rows) {
        assert.equal(row.id, `game-${expected.items[row.index].id}`);
        assert.equal(row.height, 38);
        assert.deepEqual(row.thumbnail, [48, 27]);
      }
      const metrics = await snapshot();
      assert(
        metrics.maxMountedRows <= 36 &&
          metrics.maxCachedPages <= 6 &&
          metrics.maxActiveRequests <= 1,
      );
      const perfAfter = await command("Performance.getMetrics");
      const trace = finish
        ? await finish(`${output}.${repeat}-${variant}.trace.json`)
        : null;
      // Subsequent scroll frames are reported separately from the transition trace.
      await evaluate(
        `window.__gmLayout.frames=[];window.__gmLayout.longTasks=[];document.querySelector('[role="grid"]').focus({preventScroll:true})`,
      );
      for (let i = 0; i < 12; i++) {
        const index = Math.floor(((report.info.total - 1) * i) / 11);
        await evaluate(
          `document.querySelector('.library-scroll').scrollTop=${index * 38}`,
        );
        await until(
          `document.querySelector('.game-row[aria-rowindex="${index + 2}"][aria-busy="false"]')`,
        );
      }
      await key("End", 35);
      await until(
        `document.querySelector('.game-row[aria-rowindex="${report.info.total + 1}"][aria-selected="true"][aria-busy="false"]')`,
      );
      await sleep(120);
      const scroll = await evaluate(
        `(()=>{const m=window.__gmLayout;m.running=false;cancelAnimationFrame(m.raf);m.observer.disconnect();m.mutations.disconnect();return {frames:m.frames,longTasks:m.longTasks,maxRows:m.maxRows}})()`,
      );
      const delta = Object.fromEntries(
        perfAfter.metrics.map(({ name, value }) => [
          name,
          value - (perfBefore.metrics.find((m) => m.name === name)?.value ?? 0),
        ]),
      );
      report.trials.push({
        repeat,
        variant,
        installed,
        transition,
        scroll,
        metrics,
        performanceDelta: delta,
        trace,
      });
      await writeFile(output, JSON.stringify(report, null, 2));
      console.log(
        `${repeat} ${variant}: layout=${trace?.summary.layoutWallMs.toFixed(2) ?? "untraced"}ms tasks=${transition.longTasks.map((e) => e.duration).join(",")}`,
      );
    }
  }
  const screenshot = await command("Page.captureScreenshot", { format: "png" });
  await writeFile(`${output}.png`, Buffer.from(screenshot.data, "base64"));
} finally {
  socket.close();
}
