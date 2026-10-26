// SPDX-License-Identifier: GPL-3.0-or-later
export interface Metrics {
  started: number;
  criteriaStarted?: number;
  criteriaVisibleMs?: number;
  counts: number[];
  metadataRequests: number[];
  infoMs?: number;
  usableMs?: number;
  requests: number[];
  database: number[];
  commits: number[];
  frames: number[];
  longFrames: number;
  frameCount: number;
  mounted: number;
  visible: number;
  maxMounted: number;
  active: number;
  maxActive: number;
  maxCached: number;
}
export function createMetrics(): Metrics {
  return {
    started: performance.now(),
    counts: [],
    metadataRequests: [],
    requests: [],
    database: [],
    commits: [],
    frames: [],
    longFrames: 0,
    frameCount: 0,
    mounted: 0,
    visible: 0,
    maxMounted: 0,
    active: 0,
    maxActive: 0,
    maxCached: 0,
  };
}
export function sample(list: number[], value: number) {
  if (list.length === 1024) list.shift();
  list.push(value);
}
