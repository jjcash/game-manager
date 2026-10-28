// SPDX-License-Identifier: GPL-3.0-or-later
import { expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  createManual,
  getManual,
  validateInstallation,
  validateMetadata,
} from "./manual";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const metadata = {
  title: "Valid",
  releaseDate: null,
  genre: null,
  developer: null,
  favorite: false,
};
it("validates titles and real calendar dates without IPC", () => {
  for (const title of ["", " \n", "\0", "x".repeat(201)])
    expect(() => validateMetadata({ ...metadata, title })).toThrow();
  for (const releaseDate of [
    "2023-02-29",
    "2024-02-30",
    "0000-01-01",
    "2024-13-01",
    "not-a-date",
  ])
    expect(() => validateMetadata({ ...metadata, releaseDate })).toThrow();
  expect(() =>
    validateMetadata({
      ...metadata,
      title: "日本語 🎮 %_",
      releaseDate: "2024-02-29",
    }),
  ).not.toThrow();
  expect(() =>
    validateInstallation({
      rootPathRef: "",
      platform: "windows",
      version: null,
      detectionState: "unknown",
    }),
  ).toThrow();
});
it("uses narrow create arguments and preserves structured errors", async () => {
  vi.mocked(invoke).mockResolvedValueOnce("stable-id");
  expect(await createManual(metadata, "Manual", null)).toBe("stable-id");
  expect(invoke).toHaveBeenLastCalledWith("manual_create", {
    request: { metadata, sourceName: "Manual", installation: null },
  });
  vi.mocked(invoke).mockRejectedValueOnce({
    code: "CONFLICT",
    message: "Reopen editor",
  });
  await expect(createManual(metadata, "Manual", null)).rejects.toMatchObject({
    code: "CONFLICT",
    message: "Reopen editor",
  });
  vi.mocked(invoke).mockResolvedValueOnce({ id: "wrong" });
  await expect(getManual("stable-id")).rejects.toMatchObject({
    code: "INVALID_RESPONSE",
  });
});
