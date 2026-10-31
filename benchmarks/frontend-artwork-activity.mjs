// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { connectWebview } from "./webview.mjs";
const [endpoint, output, phase] = process.argv.slice(2);
const view = await connectWebview(endpoint);
const invoke = (name, args = {}) =>
  view.evaluate(
    `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(name)},${JSON.stringify(args)})`,
  );
try {
  await view.until(
    `document.querySelector('.library-panel')?.dataset.queryReady==='true'`,
  );
  if (phase === "initialize") {
    await writeFile(
      output,
      JSON.stringify(await invoke("database_get_status"), null, 2),
      { flag: "wx" },
    );
  } else {
    const ids = (
      await invoke("library_query", {
        request: { offset: 0, limit: 100, criteria: {} },
      })
    ).items.map((r) => r.id);
    // Saturation is separate from viewport measurements; never load originals into JS.
    for (let i = 0; i < ids.length; i += 2)
      await Promise.all(
        ids
          .slice(i, i + 2)
          .map((id) => invoke("artwork_get_thumbnail", { id })),
      );
    let active = 0,
      maxActive = 0,
      activeArtwork = 0,
      maxActiveArtwork = 0,
      maxRows = 0;
    const pending = new Map();
    const events = [];
    const times = [];
    const remove = view.onEvent((m) => {
      const p = m.params;
      if (
        [
          "Network.requestWillBeSent",
          "Network.responseReceived",
          "Network.loadingFinished",
          "Network.loadingFailed",
        ].includes(m.method)
      ) {
        events.push({
          method: m.method,
          httpMethod: p.request?.method,
          id: p.requestId,
          at: p.timestamp,
          url: p.request?.url ?? p.response?.url,
        });
        if (events.length > 128) events.shift();
      }
      if (
        m.method === "Network.requestWillBeSent" &&
        p.request.method === "POST" &&
        /ipc\.localhost\/(library_query|artwork_get_thumbnail)$/.test(
          p.request.url,
        )
      ) {
        const artwork = p.request.url.endsWith("/artwork_get_thumbnail");
        pending.set(p.requestId, artwork);
        if (artwork)
          maxActiveArtwork = Math.max(maxActiveArtwork, ++activeArtwork);
        else maxActive = Math.max(maxActive, ++active);
      }
      if (
        // Response headers end the native service interval. WebView transport
        // teardown may overlap the next settled invoke and is not queue capacity.
        ["Network.responseReceived", "Network.loadingFailed"].includes(
          m.method,
        ) &&
        pending.has(p.requestId)
      ) {
        if (pending.get(p.requestId)) activeArtwork--;
        else active--;
        pending.delete(p.requestId);
      }
    });
    await view.command("Network.enable");
    // Native selection and decode run independently of the library request gate.
    await view.evaluate(
      `window.__m19Import={done:false}; window.__TAURI_INTERNALS__.invoke('artwork_choose',{request:{gameId:${JSON.stringify(ids[0])},expectedRevision:0}}).then(value=>window.__m19Import={done:true,value},error=>window.__m19Import={done:true,error}); true`,
    );
    console.log(
      "ACTIVITY: select import.png in this run's root; browsing measurement continues while import is active",
    );
    const samples = [];
    let during = 0;
    let after = 0;
    for (let i = 0; i < 2400 && after < 240; i++) {
      const start = performance.now();
      const sample = await view.evaluate(
        `(()=>{const e=document.querySelector('.library-scroll');e.scrollTop=${(i % 24) * 18 * 38};return {rows:document.querySelectorAll('.game-row').length,importDone:window.__m19Import.done,alerts:document.querySelectorAll('.library-error').length};})()`,
      );
      times.push(performance.now() - start);
      maxRows = Math.max(maxRows, sample.rows);
      if (!sample.importDone) during++;
      else after++;
      samples.push(sample);
      await view.sleep(35);
    }
    for (let i = 0; i < 600; i++) {
      if (await view.evaluate("window.__m19Import.done")) break;
      await view.sleep(100);
    }
    const imported = await view.evaluate("window.__m19Import");
    assert.equal(imported.value, true, JSON.stringify(imported));
    await view.until(
      `!document.querySelector('.library-scroll[aria-busy="true"]')`,
    );
    const native = await invoke("artwork_get_stats");
    remove();
    await view.command("Network.disable");
    const sorted = times.toSorted((a, b) => a - b);
    await writeFile(
      output,
      JSON.stringify(
        {
          phase,
          native,
          maxRows,
          maxActiveLibraryRequests: maxActive,
          maxActiveArtworkRequests: maxActiveArtwork,
          samplesDuringImport: during,
          samplesAfterImport: after,
          samples: samples.length,
          driverRoundTripMs: {
            median: sorted[Math.floor(sorted.length / 2)],
            p95: sorted[Math.floor(sorted.length * 0.95)],
            max: sorted.at(-1),
          },
          imported,
          recentTransportEvents: events,
        },
        null,
        2,
      ),
      { flag: "wx" },
    );
    assert(maxRows <= 40);
    assert(maxActive <= 1);
    assert(maxActiveArtwork <= 2);
    assert(during > 0);
    assert(after === 240);
    assert(samples.every((s) => s.alerts === 0));
    assert(
      native.memoryEntries <= 128 &&
        native.diskEntries <= 256 &&
        native.maxActive <= 2 &&
        native.maxActiveDecodes <= 2,
    );
    await writeFile(`${output}.verified`, "All activity assertions passed.\n", {
      flag: "wx",
    });
  }
} finally {
  view.socket.close();
}
