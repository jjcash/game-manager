// SPDX-License-Identifier: GPL-3.0-or-later
import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

export function distribution(values) {
  if (!values.length) return null;
  const sorted = values.toSorted((a, b) => a - b);
  const at = (p) => +sorted[Math.ceil(p * sorted.length) - 1].toFixed(3);
  return {
    n: sorted.length,
    p50: at(0.5),
    p95: at(0.95),
    p99: at(0.99),
    max: at(1),
  };
}
export function traceSummary(events) {
  const thread =
    events.find((e) => e.name.startsWith("gm:sortTrial:")) ??
    events.find((e) => e.ph === "X" && e.name === "Layout");
  const same = events.filter(
    (e) => e.ph === "X" && e.pid === thread?.pid && e.tid === thread?.tid,
  );
  const layouts = same.filter((e) => e.name === "Layout");
  const sum = (name, field = "dur") => {
    const items = same.filter((e) => e.name === name);
    if (!items.length || items.some((e) => e[field] === undefined)) return null;
    return items.reduce((n, e) => n + e[field] / 1000, 0);
  };
  return {
    layoutWallMs: sum("Layout"),
    layoutCpuMs: sum("Layout", "tdur"),
    styleWallMs: sum("UpdateLayoutTree"),
    functionWallMs: sum("FunctionCall"),
    layoutMaxMs: Math.max(0, ...layouts.map((e) => e.dur / 1000)),
    dirtyMax: Math.max(
      0,
      ...layouts.map((e) => e.args?.beginData?.dirtyObjects ?? 0),
    ),
    objectsMax: Math.max(
      0,
      ...layouts.map((e) => e.args?.beginData?.totalObjects ?? 0),
    ),
    largest: layouts
      .toSorted((a, b) => b.dur - a.dur)
      .slice(0, 3)
      .map((e) => ({
        wallMs: e.dur / 1000,
        cpuMs: e.tdur === undefined ? null : e.tdur / 1000,
        ...e.args,
      })),
    invalidations: Object.fromEntries(
      ["LayoutInvalidationTracking", "StyleRecalcInvalidationTracking"].map(
        (name) => [name, events.filter((e) => e.name === name).length],
      ),
    ),
  };
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  for (const directory of process.argv.slice(2)) {
    const names = await readdir(directory);
    const groups = new Map();
    for (const name of names.filter((n) => n.endsWith(".trace.json"))) {
      const trace = JSON.parse(await readFile(join(directory, name), "utf8"));
      const summary = traceSummary(trace.traceEvents);
      const variant =
        name.match(/\.warm-\d+-([^.]+)/)?.[1]?.replace(/^/, "warm-") ??
        name.match(/sort-\d+-([^.]+)/)?.[1] ??
        name.match(/\.\d+-([^.]+)/)?.[1] ??
        "original";
      const group = groups.get(variant) ?? [];
      group.push(summary);
      groups.set(variant, group);
    }
    const result = Object.fromEntries(
      [...groups].map(([variant, items]) => [
        variant,
        Object.fromEntries(
          [
            "layoutWallMs",
            "layoutCpuMs",
            "layoutMaxMs",
            "dirtyMax",
            "objectsMax",
            "styleWallMs",
            "functionWallMs",
          ].map((k) => [
            k,
            distribution(items.map((s) => s[k]).filter((v) => v !== null)),
          ]),
        ),
      ]),
    );
    const observations = [];
    for (const name of names.filter(
      (n) =>
        /^\d+.*\.json$/.test(n) &&
        !n.endsWith(".trace.json") &&
        !/\.(memory|environment|memory-series|audit)\.json$/.test(n),
    )) {
      const r = JSON.parse(
        (await readFile(join(directory, name), "utf8")).replace(/^\uFEFF/, ""),
      );
      for (const t of r.layoutTrials ?? [])
        observations.push({
          file: name,
          repetition: t.repetition,
          variant: t.variant,
          frames: distribution(t.dom.frames),
          longTasks: t.dom.longTasks,
          metrics: t.metrics,
          performanceDelta: t.performanceDelta,
          cjkFonts: t.cjkFonts,
          fontWarmup: t.fontWarmup,
        });
    }
    console.log(JSON.stringify({ directory, result, observations }, null, 2));
  }
}
