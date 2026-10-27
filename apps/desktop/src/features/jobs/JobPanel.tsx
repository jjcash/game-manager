// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useState } from "react";
import {
  cancelJob,
  listJobs,
  startJob,
  terminal,
  type JobReport,
  type Workload,
} from "./api";
export function JobPanel() {
  const [report, setReport] = useState<JobReport>();
  const [kind, setKind] = useState<Workload["kind"]>("mixed");
  const [records, setRecords] = useState(1000);
  const [intensity, setIntensity] = useState<Workload["intensity"]>("gentle");
  const [error, setError] = useState<string>();
  const [pending, setPending] = useState(false);
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      try {
        const current = await listJobs();
        if (!stopped) {
          setReport(current);
          setError(undefined);
        }
      } catch (e) {
        if (!stopped) setError(String(e));
      } finally {
        if (!stopped) timer = setTimeout(() => void poll(), 500);
      }
    };
    void poll();
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  }, [retry]);
  async function start(fail = false) {
    setPending(true);
    setError(undefined);
    try {
      await startJob({
        kind: fail ? "write" : kind,
        records: fail ? 128 : records,
        seed: 20261026,
        intensity,
        fault: fail ? "write" : "none",
      });
      setReport(await listJobs());
    } catch (e) {
      setError(String(e));
    } finally {
      setPending(false);
    }
  }
  async function cancel(id: number) {
    try {
      await cancelJob(id);
      setReport(await listJobs());
    } catch (e) {
      setError(String(e));
    }
  }
  return (
    <details
      className="synthetic-panel"
      aria-label="Synthetic background testing"
    >
      <summary>Synthetic background testing · disposable fixture only</summary>
      <p>
        No stores or game folders are accessed. Writes affect this isolated
        benchmark database.
      </p>
      <label>
        Workload{" "}
        <select
          aria-label="Synthetic workload"
          value={kind}
          onChange={(e) => setKind(e.target.value as Workload["kind"])}
        >
          <option value="cpu">Discovery / parsing CPU</option>
          <option value="hash">Fixture hashing</option>
          <option value="write">SQLite batches</option>
          <option value="mixed">
            Parsing + hashing + writes + artwork CPU
          </option>
        </select>
      </label>
      <label>
        Records{" "}
        <select
          aria-label="Synthetic records"
          value={records}
          onChange={(e) => setRecords(Number(e.target.value))}
        >
          <option>1000</option>
          <option>10000</option>
          <option>100000</option>
        </select>
      </label>
      <label>
        Intensity{" "}
        <select
          aria-label="Synthetic intensity"
          value={intensity}
          onChange={(e) =>
            setIntensity(e.target.value as Workload["intensity"])
          }
        >
          <option value="gentle">Gentle yields</option>
          <option value="saturated">Saturated</option>
          <option value="slow">Slow cancellable I/O</option>
        </select>
      </label>
      <button
        className="check-button"
        disabled={
          pending ||
          (report?.jobs.filter((j) => !terminal(j.state)).length ?? 0) >= 3
        }
        onClick={() => void start()}
      >
        Start synthetic job
      </button>
      <button
        className="check-button"
        disabled={pending}
        onClick={() => void start(true)}
      >
        Test rolled-back failure
      </button>
      {error && (
        <p role="alert">
          Synthetic controls: {error}{" "}
          <button onClick={() => setRetry((n) => n + 1)}>
            Retry job status
          </button>
        </p>
      )}
      <p role="status">
        {report
          ? `${report.active} active · ${report.queued}/2 queued · ${report.scheduling.workersActive}/2 resource workers · ${report.scheduling.backoffs} backoffs`
          : "Loading job status…"}
      </p>
      <ul>
        {report?.jobs.map((job) => (
          <li key={job.id} data-job-id={job.id} data-job-state={job.state}>
            <strong>
              Job {job.id} · {job.state}
            </strong>{" "}
            <progress
              max={job.request.records}
              value={job.processed}
              aria-label={`Job ${job.id} progress`}
            />{" "}
            {job.processed}/{job.request.records} processed · {job.committed}{" "}
            committed · {Math.round(job.elapsedMs)} ms
            {job.error && <span> · {job.error}</span>}
            {!terminal(job.state) && (
              <button
                className="check-button"
                disabled={
                  job.state === "cancelRequested" || job.state === "finalizing"
                }
                onClick={() => void cancel(job.id)}
              >
                Cancel job {job.id}
              </button>
            )}
          </li>
        ))}
      </ul>
    </details>
  );
}
