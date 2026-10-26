// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { getLibraryInfo, getLibraryWindow } from "./library";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
afterEach(() => vi.resetAllMocks());
describe("library IPC", () => {
  it("validates requests before invoking and rejects malformed/oversized responses", async () => {
    for (const [offset, limit] of [
      [-1, 100],
      [0, 201],
      [0, 0],
      [1.5, 10],
      [1_000_001, 10],
    ])
      await expect(getLibraryWindow(offset, limit)).rejects.toMatchObject({
        code: "INVALID_REQUEST",
      });
    expect(invoke).not.toHaveBeenCalled();
    vi.mocked(invoke).mockResolvedValue({
      offset: 0,
      items: Array(101).fill({}),
      queryMs: 1,
    });
    await expect(getLibraryWindow(0, 100)).rejects.toMatchObject({
      code: "INVALID_RESPONSE",
    });
    vi.mocked(invoke).mockResolvedValue({
      total: -1,
      source: "test",
      diagnostics: true,
    });
    await expect(getLibraryInfo()).rejects.toMatchObject({
      code: "INVALID_RESPONSE",
    });
  });
  it("distinguishes empty data, structured failure and connection failure", async () => {
    vi.mocked(invoke)
      .mockResolvedValueOnce({
        total: 0,
        libraryTotal: 0,
        sources: [],
        queryMs: 0,
        source: "Local",
        diagnostics: false,
      })
      .mockRejectedValueOnce({ code: "MIGRATION", message: "Preserved" })
      .mockRejectedValueOnce("offline");
    expect((await getLibraryInfo()).total).toBe(0);
    await expect(getLibraryInfo()).rejects.toMatchObject({ code: "MIGRATION" });
    await expect(getLibraryInfo()).rejects.toMatchObject({
      code: "CONNECTION",
    });
  });
});
