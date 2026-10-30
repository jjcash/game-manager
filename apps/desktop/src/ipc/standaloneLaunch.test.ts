// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { launchStandalone } from "./standaloneLaunch";
import type { Details } from "./manual";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(() => vi.clearAllMocks());
const d = { id: "a".repeat(32), revision: 8 } as Details;
const p = { id: "b".repeat(32), revision: 3 } as Details["profiles"][number];
it("sends precisely the persisted identifiers and revisions", async () => {
  vi.mocked(invoke).mockResolvedValue({ pid: 42 });
  expect(await launchStandalone(d, p)).toEqual({ pid: 42 });
  expect(invoke).toHaveBeenCalledExactlyOnceWith("standalone_launch", {
    request: {
      gameId: d.id,
      profileId: p.id,
      expectedProfileRevision: 3,
      expectedGameRevision: 8,
    },
  });
});
it.each([
  "missingExecutable",
  "invalidWorkingDirectory",
  "staleContext",
  "databaseUnavailable",
  "stopping",
  "permissionDenied",
  "osFailure",
  "elevatedParent",
])("sanitizes %s without retries", async (code) => {
  vi.mocked(invoke).mockRejectedValue({
    stage: "creation",
    code,
    message: "C:\\private\\secret.exe --secret",
    arguments: ["secret"],
  });
  await expect(launchStandalone(d, p)).rejects.not.toThrow(/private|secret/);
  expect(invoke).toHaveBeenCalledTimes(1);
});
it("does not retry a malformed success or lost response", async () => {
  vi.mocked(invoke).mockResolvedValue({ pid: 0 });
  await expect(launchStandalone(d, p)).rejects.toMatchObject({
    code: "outcomeUnknown",
  });
  expect(invoke).toHaveBeenCalledTimes(1);
});
