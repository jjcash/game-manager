// SPDX-License-Identifier: GPL-3.0-or-later
import { expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { backupStatus, createBackup, snapshot } from "./backup";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
it("rejects malformed and unbounded maintenance responses", async () => {
  for (const invalid of [
    null,
    {},
    { formatVersion: 99 },
    {
      formatVersion: 1,
      applicationVersion: "v",
      schemaVersion: 2,
      bytes: 1,
      counts: { games: Infinity },
    },
  ])
    expect(() => snapshot(invalid)).toThrow();
  vi.mocked(invoke).mockResolvedValue({
    available: true,
    restartRequired: false,
    outcome: "successWithoutVerification",
    candidate: null,
  });
  await expect(backupStatus()).rejects.toThrow();
  vi.mocked(invoke).mockResolvedValue(null);
  expect(await createBackup()).toBeNull();
});
