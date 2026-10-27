// SPDX-License-Identifier: GPL-3.0-or-later
import { invoke } from "@tauri-apps/api/core";
export type JobState =
  | "queued"
  | "running"
  | "cancelRequested"
  | "finalizing"
  | "cancelled"
  | "completed"
  | "failed"
  | "interrupted";
export interface Workload {
  kind: "cpu" | "hash" | "write" | "mixed";
  records: number;
  seed: number;
  intensity: "gentle" | "saturated" | "slow";
  fault: "none" | "parse" | "write" | "resource";
}
export interface Job {
  id: number;
  state: JobState;
  request: Workload;
  processed: number;
  committed: number;
  batches: number;
  digest: string;
  elapsedMs: number;
  cancelLatencyMs: number | null;
  error: string | null;
  busyRetries: number;
  progressRevision: number;
}
export interface JobReport {
  jobs: Job[];
  active: number;
  queued: number;
  queueLimit: number;
  cpuLimit: number;
  scheduling: { workersActive: number; workersMax: number; backoffs: number };
}
export const terminal = (state: JobState) =>
  ["completed", "cancelled", "failed", "interrupted"].includes(state);
function validate(job: Job) {
  if (
    job?.state === "completed" &&
    (job.processed !== job.request?.records ||
      (["write", "mixed"].includes(job.request?.kind) &&
        job.committed !== job.request.records))
  )
    throw new Error("Invalid synthetic job completion");
  if (
    !job ||
    !Number.isSafeInteger(job.id) ||
    job.id < 1 ||
    ![
      "queued",
      "running",
      "cancelRequested",
      "finalizing",
      "cancelled",
      "completed",
      "failed",
      "interrupted",
    ].includes(job.state) ||
    !job.request ||
    !Number.isSafeInteger(job.request.records) ||
    job.request.records < 1 ||
    job.request.records > 100000 ||
    !Number.isSafeInteger(job.processed) ||
    job.processed < 0 ||
    job.processed > job.request.records ||
    !Number.isSafeInteger(job.committed) ||
    job.committed < 0 ||
    job.committed > job.processed
  )
    throw new Error("Invalid synthetic job response");
  return job;
}
export async function listJobs(): Promise<JobReport> {
  const report = await invoke<JobReport>("synthetic_list");
  if (
    !report ||
    !Array.isArray(report.jobs) ||
    report.jobs.length > 8 ||
    report.active > 1 ||
    report.queued > 2 ||
    !report.scheduling
  )
    throw new Error("Invalid synthetic job snapshot");
  report.jobs.forEach(validate);
  return report;
}
export async function startJob(request: Workload) {
  return validate(await invoke<Job>("synthetic_start", { request }));
}
export async function cancelJob(id: number) {
  return validate(await invoke<Job>("synthetic_cancel", { id }));
}
