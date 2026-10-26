// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import fixture from "./app-status.fixture.json";
import { getAppStatus } from "./app";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("app_get_status boundary", () => {
  it("accepts the shared Rust serialization contract", async () => {
    vi.mocked(invoke).mockResolvedValueOnce(fixture);
    await expect(getAppStatus()).resolves.toEqual(fixture);
    expect(invoke).toHaveBeenCalledWith("app_get_status");
  });
  it.each([
    null,
    {},
    { ...fixture, backend: "unknown" },
    { ...fixture, version: 1 },
  ])("rejects a malformed response: %j", async (response) => {
    vi.mocked(invoke).mockResolvedValueOnce(response);
    await expect(getAppStatus()).rejects.toThrow("unexpected status");
  });
  it("normalizes IPC failures without exposing raw backend errors", async () => {
    vi.mocked(invoke).mockRejectedValueOnce("private OS diagnostic");
    await expect(getAppStatus()).rejects.toThrow(
      "Could not reach the desktop backend",
    );
  });
});
