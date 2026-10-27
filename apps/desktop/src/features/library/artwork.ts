// SPDX-License-Identifier: GPL-3.0-or-later
import { invoke } from "@tauri-apps/api/core";

export interface ArtworkResult {
  status: string;
  dataUrl: string | null;
  cache: string;
  workMs: number;
  decodeMs: number;
}
type Listener = (result: ArtworkResult) => void;
interface Job {
  id: string;
  listeners: Map<Listener, number>;
  active: boolean;
}
const failed: ArtworkResult = {
  status: "failed",
  dataUrl: null,
  cache: "none",
  workMs: 0,
  decodeMs: 0,
};
export async function getThumbnail(id: string): Promise<ArtworkResult> {
  if (!/^[a-zA-Z0-9-]{1,128}$/.test(id)) return failed;
  const v = await invoke<ArtworkResult>("artwork_get_thumbnail", { id });
  if (
    !v ||
    typeof v.status !== "string" ||
    typeof v.cache !== "string" ||
    !Number.isFinite(v.workMs) ||
    !Number.isFinite(v.decodeMs) ||
    (v.dataUrl !== null &&
      (typeof v.dataUrl !== "string" ||
        v.dataUrl.length > 22000 ||
        !/^data:image\/png;base64,[A-Za-z0-9+/=]+$/.test(v.dataUrl)))
  )
    throw new Error("Invalid thumbnail response");
  return v;
}
/** One app-wide bounded scheduler; independent of the library request gate.
 * Consumers release work as soon as virtual rows disappear. In-flight native work
 * retains its slot until settlement, but cannot update a released consumer.
 */
export class ArtworkQueue {
  private concurrency = 2;
  configure(workers: number) {
    if (![0, 1, 2].includes(workers))
      throw new Error("Invalid artwork worker limit");
    this.concurrency = workers;
    this.schedule();
  }
  private jobs = new Map<string, Job>();
  private cache = new Map<string, { result: ArtworkResult; at: number }>();
  private timer?: ReturnType<typeof setTimeout>;
  active = 0;
  maxActive = 0;
  maxQueued = 0;
  hits = 0;
  cancelled = 0;
  dropped = 0;
  requests: {
    at: number;
    duration: number;
    cache: string;
    decodeMs: number;
    status: string;
  }[] = [];
  constructor(private load = getThumbnail) {}
  get queued() {
    return [...this.jobs.values()].filter((j) => !j.active).length;
  }
  get cached() {
    return this.cache.size;
  }
  clear() {
    this.cache.clear();
  }
  subscribe(id: string, priority: number, listener: Listener) {
    if (this.concurrency === 0) {
      listener({ ...failed, status: "missing" });
      return () => {};
    }
    const entry = this.cache.get(id);
    if (
      entry &&
      performance.now() - entry.at < (entry.result.dataUrl ? 5000 : 2000)
    ) {
      this.cache.delete(id);
      this.cache.set(id, entry);
      this.hits++;
      listener(entry.result);
      return () => {};
    }
    this.cache.delete(id);
    let job = this.jobs.get(id);
    if (!job) {
      if (this.queued >= 32) {
        const worst = [...this.jobs.values()]
          .filter((j) => !j.active)
          .sort(
            (a, b) =>
              Math.min(...b.listeners.values()) -
              Math.min(...a.listeners.values()),
          )[0];
        if (worst && Math.min(...worst.listeners.values()) > priority) {
          this.jobs.delete(worst.id);
          worst.listeners.forEach((_p, fn) => fn(failed));
        } else {
          this.dropped++;
          listener(failed);
          return () => {};
        }
      }
      job = { id, listeners: new Map(), active: false };
      this.jobs.set(id, job);
    }
    job.listeners.set(listener, priority);
    this.maxQueued = Math.max(this.maxQueued, this.queued);
    this.schedule();
    const subscribed = job;
    return () => {
      subscribed.listeners.delete(listener);
      if (!subscribed.listeners.size && !subscribed.active) {
        this.jobs.delete(id);
        this.cancelled++;
      }
    };
  }
  private schedule() {
    if (this.timer !== undefined) return;
    // Batch row effects, so visible requests sort ahead of overscan requests.
    this.timer = setTimeout(() => {
      this.timer = undefined;
      this.pump();
    }, 16);
  }
  private pump() {
    while (this.active < this.concurrency) {
      const job = [...this.jobs.values()]
        .filter((j) => !j.active && j.listeners.size)
        .sort(
          (a, b) =>
            Math.min(...a.listeners.values()) -
            Math.min(...b.listeners.values()),
        )[0];
      if (!job) return;
      job.active = true;
      this.active++;
      this.maxActive = Math.max(this.maxActive, this.active);
      const started = performance.now();
      void this.load(job.id)
        .catch(() => failed)
        .then((result) => {
          this.requests.push({
            at: started,
            duration: performance.now() - started,
            cache: result.cache,
            decodeMs: result.decodeMs,
            status: result.status,
          });
          if (this.requests.length > 1024) this.requests.shift();
          this.cache.delete(job.id);
          this.cache.set(job.id, { result, at: performance.now() });
          while (this.cache.size > 128)
            this.cache.delete(this.cache.keys().next().value!);
          this.jobs.delete(job.id);
          this.active--;
          job.listeners.forEach((_p, listener) => listener(result));
          this.schedule();
        });
    }
  }
}
export const artworkQueue = new ArtworkQueue();
