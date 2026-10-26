// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useState } from "react";
import type { QueryClient } from "@tanstack/react-query";
import { sample, type Metrics } from "./metrics";
import { pageGate } from "./requestGate";
function distribution(values: number[]) {
  if (!values.length) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const at = (p: number) =>
    +sorted[
      Math.min(sorted.length - 1, Math.ceil(sorted.length * p) - 1)
    ].toFixed(2);
  return {
    samples: sorted.length,
    median: at(0.5),
    p95: at(0.95),
    p99: at(0.99),
    max: +sorted[sorted.length - 1].toFixed(2),
  };
}
export function Diagnostics({
  metrics: m,
  client,
  source,
  total,
}: {
  metrics: Metrics;
  client: QueryClient;
  source: string;
  total: number;
}) {
  const [capture, setCapture] = useState(false);
  const [, update] = useState(0);
  useEffect(() => {
    const timer = setInterval(() => update((n) => n + 1), 1000);
    return () => clearInterval(timer);
  }, []);
  useEffect(() => {
    if (!capture) return;
    let frame = 0;
    let previous = 0;
    const tick = (now: number) => {
      if (previous && document.visibilityState === "visible") {
        const duration = now - previous;
        sample(m.frames, duration);
        m.frameCount++;
        if (duration > 50) m.longFrames++;
      }
      previous = now;
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [capture, m]);
  const pages = client.getQueryCache().findAll({ queryKey: ["page"] });
  const heap = (
    performance as Performance & { memory?: { usedJSHeapSize: number } }
  ).memory?.usedJSHeapSize;
  const report = {
    source,
    total,
    viewport: [innerWidth, innerHeight],
    devicePixelRatio,
    initialInfoMs: m.infoMs,
    firstUsableMs: m.usableMs,
    mountedRows: m.mounted,
    visibleRows: m.visible,
    overscanRows: m.mounted - m.visible,
    maxMountedRows: m.maxMounted,
    activeRequests: pageGate.activeRequests,
    queuedRequests: pageGate.queued,
    activePages: pages.filter((q) => q.getObserversCount() > 0).length,
    maxActiveRequests: m.maxActive,
    cachedPages: pages.length,
    maxCachedPages: m.maxCached,
    requestMs: distribution(m.requests),
    databaseMs: distribution(m.database),
    reactRenderMs: distribution(m.commits),
    reactCommits: m.commits.length,
    frameIntervalMs: distribution(m.frames),
    capturedFrames: m.frameCount,
    framesOver50Ms: m.longFrames,
    jsHeapBytes: heap ?? "unavailable",
    applicationMemory: "measure externally; WebView heap is not process memory",
  };
  return (
    <details className="diagnostics">
      <summary>
        Development diagnostics · {total.toLocaleString()} games
      </summary>
      <p>{source}</p>
      <button className="check-button" onClick={() => setCapture(!capture)}>
        {capture ? "Stop frame capture" : "Start frame capture"}
      </button>
      <p>
        Frame intervals are requestAnimationFrame samples, not measured FPS.
        Last 1,024 samples per metric; hidden tabs excluded. React profiling
        requires a development build.
      </p>
      <pre aria-label="Performance measurements">
        {JSON.stringify(report, null, 2)}
      </pre>
    </details>
  );
}
