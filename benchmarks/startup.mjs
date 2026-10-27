// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { readFile, readdir } from "node:fs/promises";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";

export function distribution(values) {
  const a = values.filter(Number.isFinite).sort((a, b) => a - b);
  if (!a.length) return null;
  const n = a.length;
  return {
    n,
    median: (a[Math.floor((n - 1) / 2)] + a[Math.floor(n / 2)]) / 2,
    p95: a[Math.ceil(n * 0.95) - 1],
    max: a[n - 1],
  };
}

export function derive(report, external, marks) {
  assert.equal(report.scenario, "startup");
  assert(!report.failure, report.failure);
  assert(Number.isFinite(report.startup.firstUsable));
  assert(
    Number.isFinite(report.initial.firstUsableMs) &&
      report.initial.firstUsableMs >= 0,
  );
  assert(
    report.initial.total > 0 && report.initial.visibleRows > 0,
    "Expected a populated library with visible real rows",
  );
  assert(Number.isFinite(external.createdEpochMs));
  const origin = report.timeOrigin - external.createdEpochMs;
  const stages = {};
  for (const m of marks) {
    const old = stages[m.startupStage];
    if (
      !old ||
      (old.epochMs === m.epochMs &&
        !Number.isFinite(old.elapsedMs) &&
        Number.isFinite(m.elapsedMs))
    )
      stages[m.startupStage] = m;
  }
  const native = (name) =>
    stages[name] ? stages[name].epochMs - external.createdEpochMs : null;
  // Native phase differences prefer the one-process monotonic clock. Old
  // artifacts have only UTC, so their differences keep wall-clock uncertainty.
  const phase = (a, b) => {
    if (!stages[a] || !stages[b]) return null;
    const clock =
      Number.isFinite(stages[a].elapsedMs) &&
      Number.isFinite(stages[b].elapsedMs)
        ? "elapsedMs"
        : "epochMs";
    return stages[b][clock] - stages[a][clock];
  };
  const rows = origin + report.startup.firstUsable;
  assert(rows >= 0);
  assert(external.visibleUpperMs >= external.visibleLowerMs);
  const first = (name) =>
    report.startup[name]?.length ? origin + report.startup[name][0] : null;
  return {
    profile: external.profile,
    processToMain: native("main-entry"),
    processToBackend: native("backend-published"),
    processToShell: first("shellCommit"),
    processToFrontend: first("frontendEntry"),
    processToNavigation: origin,
    processToRows: rows,
    navigationToRows: report.startup.firstUsable,
    backendToRows:
      native("backend-published") === null
        ? null
        : rows - native("backend-published"),
    processToImages:
      report.startup.visibleThumbnailReady > 0
        ? origin + report.startup.visibleThumbnailReady
        : null,
    visibleLower: external.visibleLowerMs,
    visibleUpper: external.visibleUpperMs,
    visibleToRowsLower: rows - external.visibleUpperMs,
    visibleToRowsUpper: rows - external.visibleLowerMs,
    mainToContext: phase("main-entry", "context-complete"),
    contextToRuntime: phase("context-complete", "runtime-built"),
    runtimeToSetup: phase("runtime-built", "setup-entry"),
    windowBuild: phase("window-build-entry", "window-build-complete"),
    applicationSetup: phase("application-setup-entry", "setup-complete"),
    readonlyPool: phase("readonly-open-entry", "readonly-pool-opened"),
    artworkOpen: phase("application-setup-entry", "artwork-open-complete"),
    directories: phase("artwork-open-complete", "directories-resolved"),
    readonlyIdentity: phase(
      "readonly-pool-opened",
      "readonly-identity-migrations-validated",
    ),
    readonlyCountValidation: phase(
      "readonly-identity-migrations-validated",
      "readonly-count-validated",
    ),
    readonlyValidation: phase(
      "readonly-pool-opened",
      "readonly-count-validated",
    ),
    databaseTask: phase("database-task-entry", "backend-published"),
    runLog: phase("run-entry", "run-entry-log-complete"),
    backendLog: phase("backend-published", "backend-published-log-complete"),
    metadata: phase("metadata-request-entry", "metadata-request-complete"),
    page: phase("window-request-entry", "window-request-complete"),
    metadataIpc: report.initial.metadataRequestMs?.median,
    countService: report.initial.databaseCountMs?.median,
    pageIpc: report.initial.requestMs?.median,
    pageService: report.initial.databaseMs?.median,
    nativeStages: marks,
    browserStartup: report.startup,
  };
}

export async function summarize(directory) {
  const launches = [];
  for (const f of await readdir(directory)) {
    if (!/^\d+-(idle|art2)-\d+\.json$/.test(f)) continue;
    const path = join(directory, f);
    const report = JSON.parse(await readFile(path, "utf8"));
    const external = JSON.parse(
      (await readFile(path + ".startup.json", "utf8")).replace(/^\uFEFF/, ""),
    );
    const marks = (await readFile(path + ".app.stderr.txt", "utf8"))
      .split(/\r?\n/)
      .filter((s) => s.startsWith("{") && s.includes('"startupStage"'))
      .map((s) => JSON.parse(s));
    const [size, mode] = f.split("-");
    const optionalJson = async (suffix) => {
      try {
        return JSON.parse(
          (await readFile(path + suffix, "utf8")).replace(/^\uFEFF/, ""),
        );
      } catch (error) {
        if (error.code === "ENOENT") return null;
        throw error;
      }
    };
    const memory = await optionalJson(".memory.json");
    const resources = await optionalJson(".resources.json");
    launches.push({
      memoryPoint: memory,
      resourcePoint: resources,
      treePrivateMiB: memory ? memory.privateBytes / 1048576 : null,
      nativePrivateMiB: memory ? memory.nativePrivateBytes / 1048576 : null,
      nativeCpuAtCollectionMs: resources?.nativeCpuMs ?? null,
      file: f,
      size: Number(size),
      mode,
      ...derive(report, external, marks),
    });
  }
  const groups = {};
  for (const launch of launches) {
    const key = `${launch.size}/${launch.mode}/${launch.profile}`;
    (groups[key] ??= []).push(launch);
  }
  const summaries = Object.fromEntries(
    Object.entries(groups).map(([k, items]) => [
      k,
      Object.fromEntries(
        Object.keys(items[0])
          .filter((key) => items.some((i) => typeof i[key] === "number"))
          .map((key) => [key, distribution(items.map((i) => i[key]))]),
      ),
    ]),
  );
  return { directory, launchCount: launches.length, summaries, launches };
}
if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  console.log(
    JSON.stringify(await summarize(resolve(process.argv[2])), null, 2),
  );
}
