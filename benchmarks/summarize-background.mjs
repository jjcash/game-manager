// SPDX-License-Identifier: GPL-3.0-or-later
// Compact derived evidence; raw reports remain the source of truth.
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
const percentile = (values, p) => {
  const sorted = values.filter(Number.isFinite).sort((a, b) => a - b);
  return sorted.length
    ? sorted[Math.max(0, Math.ceil(sorted.length * p) - 1)]
    : null;
};
const stats = (values) => ({
  n: values.length,
  p50: percentile(values, 0.5),
  p95: percentile(values, 0.95),
  max: percentile(values, 1),
});
const result = [];
for (const directory of process.argv.slice(2)) {
  for (const file of await readdir(directory)) {
    if (
      !/^\d+-(idle|cpu|hash|write|art1|art2|mixed0|mixed1|mixed2|priority)-\d+\.json$/.test(
        file,
      )
    )
      continue;
    const r = JSON.parse(await readFile(path.join(directory, file), "utf8"));
    result.push({
      file: path.join(directory, file),
      games: r.initial?.total,
      mode: r.mode,
      failure: r.failure ?? null,
      usable: r.initial?.firstUsableMs,
      artworkReady: r.initialVisibleArtworkMs,
      cases: r.cases.map((c) => ({
        name: c.name,
        metadata: stats(c.metadata),
        window: stats(c.windows),
        count: stats(c.counts),
        page: stats(c.pages),
        countPage: stats(c.counts.map((v, i) => v + c.pages[i])),
        ipcPair: stats(c.metadata.map((v, i) => v + c.windows[i])),
        countPool: stats(c.countPool),
        pagePool: stats(c.pagePool),
        activeSamples: c.load.filter((l) => l.active > 0).length,
        samples: c.load.length,
      })),
      ui: r.ui.map((u) => ({
        name: u.name,
        visible: u.metrics.criteriaToVisibleMs,
        queue: u.metrics.foregroundQueueWaitMs,
        page: u.metrics.requestMs,
      })),
      phases: Object.fromEntries(
        Object.entries(r.frames?.phases ?? {}).map(([name, p]) => [
          name,
          { ...stats(p.samples), total: p.total, over50: p.over50, max: p.max },
        ]),
      ),
      longTasks: r.frames?.longTasks,
      inputs: stats((r.frames?.inputs ?? []).map((i) => i.duration)),
      completion: r.completion.map((j) => ({
        kind: j.request.kind,
        records: j.processed,
        committed: j.committed,
        ms: j.elapsedMs,
        recordsPerSecond: (j.processed * 1000) / j.elapsedMs,
        transaction: stats(j.transactionMs),
        pool: stats(j.poolWaitMs),
        busy: j.busyRetries,
      })),
      cancel: r.cancel.map((c) => ({
        observed: c.observedMs,
        native: c.final.cancelLatencyMs,
        processed: c.final.processed,
        committed: c.final.committed,
      })),
      load: r.load,
      resource: r.finalJobs?.scheduling,
      artwork: r.artwork,
      memoryCycles: r.memoryCycles,
    });
  }
}
console.log(JSON.stringify(result, null, 2));
