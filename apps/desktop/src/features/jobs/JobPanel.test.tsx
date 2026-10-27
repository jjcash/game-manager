// SPDX-License-Identifier: GPL-3.0-or-later
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { JobPanel } from "./JobPanel";
import { cancelJob, listJobs, startJob, type Job, type JobReport } from "./api";
vi.mock("./api", async (original) => ({
  ...(await original<typeof import("./api")>()),
  listJobs: vi.fn(),
  startJob: vi.fn(),
  cancelJob: vi.fn(),
}));
const job = (state: Job["state"]): Job => ({
  id: 1,
  state,
  request: {
    kind: "write",
    records: 128,
    seed: 123,
    intensity: "gentle",
    fault: "none",
  },
  processed: 64,
  committed: 64,
  batches: 1,
  digest: "123",
  elapsedMs: 20,
  cancelLatencyMs: null,
  error: null,
  busyRetries: 0,
  progressRevision: 1,
});
const report = (state: Job["state"]): JobReport => ({
  jobs: [job(state)],
  active: state === "running" ? 1 : 0,
  queued: state === "queued" ? 1 : 0,
  queueLimit: 2,
  cpuLimit: 2,
  scheduling: { workersActive: 0, workersMax: 1, backoffs: 0 },
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.resetAllMocks();
});
it("starts a typed predefined workload and requests cooperative cancellation", async () => {
  vi.mocked(listJobs).mockResolvedValue(report("running"));
  vi.mocked(startJob).mockResolvedValue(job("queued"));
  vi.mocked(cancelJob).mockResolvedValue(job("cancelRequested"));
  render(<JobPanel />);
  await screen.findByText("Job 1 · running");
  fireEvent.click(screen.getByRole("button", { name: "Start synthetic job" }));
  await waitFor(() =>
    expect(startJob).toHaveBeenCalledWith({
      kind: "mixed",
      records: 1000,
      seed: 20261026,
      intensity: "gentle",
      fault: "none",
    }),
  );
  vi.mocked(listJobs).mockResolvedValue(report("cancelRequested"));
  fireEvent.click(screen.getByRole("button", { name: "Cancel job 1" }));
  await screen.findByText("Job 1 · cancelRequested");
  expect(cancelJob).toHaveBeenCalledWith(1);
  expect(
    screen
      .getByRole("button", { name: "Cancel job 1" })
      .hasAttribute("disabled"),
  ).toBe(true);
});
it("renders partial failure distinctly, allows a fresh restart, and retries status failures", async () => {
  vi.mocked(listJobs)
    .mockRejectedValueOnce(new Error("status offline"))
    .mockResolvedValue({
      ...report("failed"),
      jobs: [
        {
          ...job("failed"),
          processed: 128,
          error: "SIMULATED_WRITE_FAILURE_ROLLED_BACK",
        },
      ],
    });
  vi.mocked(startJob).mockResolvedValue(job("queued"));
  render(<JobPanel />);
  await screen.findByRole("alert");
  fireEvent.click(screen.getByRole("button", { name: "Retry job status" }));
  await screen.findByText("Job 1 · failed");
  expect(screen.getByText(/128\/128 processed/)).toBeTruthy();
  expect(screen.getByText(/64 committed/)).toBeTruthy();
  fireEvent.click(
    screen.getByRole("button", { name: "Test rolled-back failure" }),
  );
  await waitFor(() =>
    expect(startJob).toHaveBeenCalledWith(
      expect.objectContaining({ kind: "write", records: 128, fault: "write" }),
    ),
  );
});
it("serializes status polling and discards late responses after unmount", async () => {
  vi.useFakeTimers();
  let finish!: (value: JobReport) => void;
  vi.mocked(listJobs).mockReturnValue(
    new Promise((resolve) => {
      finish = resolve;
    }),
  );
  const view = render(<JobPanel />);
  await act(() => vi.advanceTimersByTimeAsync(2000));
  expect(listJobs).toHaveBeenCalledTimes(1);
  view.unmount();
  await act(async () => {
    finish(report("completed"));
    await vi.advanceTimersByTimeAsync(2000);
  });
  expect(listJobs).toHaveBeenCalledTimes(1);
});
