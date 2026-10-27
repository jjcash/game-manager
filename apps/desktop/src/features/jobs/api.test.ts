// SPDX-License-Identifier: GPL-3.0-or-later
import { invoke } from "@tauri-apps/api/core";
import { afterEach, expect, it, vi } from "vitest";
import { listJobs, startJob } from "./api";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(() => vi.resetAllMocks());
it("rejects unbounded or inconsistent status payloads", async () => {
  vi.mocked(invoke).mockResolvedValue({
    jobs: Array.from({ length: 9 }),
    active: 0,
    queued: 0,
    scheduling: {},
  });
  await expect(listJobs()).rejects.toThrow("Invalid synthetic job snapshot");
  vi.mocked(invoke).mockResolvedValue({
    jobs: [
      {
        id: 1,
        state: "completed",
        request: { records: 100 },
        processed: 101,
        committed: 101,
      },
    ],
    active: 0,
    queued: 0,
    scheduling: {},
  });
  await expect(listJobs()).rejects.toThrow("Invalid synthetic job completion");
});
it("uses only a typed synthetic request command, without path or SQL arguments", async () => {
  const request = {
    kind: "write" as const,
    records: 64,
    seed: 123,
    intensity: "gentle" as const,
    fault: "none" as const,
  };
  vi.mocked(invoke).mockResolvedValue({
    id: 1,
    state: "queued",
    request,
    processed: 0,
    committed: 0,
  });
  await startJob(request);
  expect(invoke).toHaveBeenCalledWith("synthetic_start", { request });
});
