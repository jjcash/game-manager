// SPDX-License-Identifier: GPL-3.0-or-later
import { expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  createManual,
  getManual,
  validateInstallation,
  validateMetadata,
  validateGroupName,
  mutateManual,
  type Details,
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

it("validates bounded group associations and sends stable IDs with the game revision", async () => {
  for (const name of ["", " ", "bad\u0000name", "x".repeat(81)])
    expect(() => validateGroupName(name)).toThrow();
  expect(() => validateGroupName("日本語 🎮")).not.toThrow();
  const value: Details = {
    id: "game",
    revision: 7,
    origin: "manual",
    metadata,
    overriddenFields: [],
    entries: [
      {
        id: "e",
        sourceId: "s",
        sourceName: "Manual",
        sourceType: "manual",
        completionStatus: "not_started",
      },
    ],
    installations: [
      {
        id: "i",
        entryId: "e",
        rootPathRef: "opaque",
        platform: "windows",
        version: null,
        detectionState: "unknown",
        stateOrigin: "legacy",
        groupId: "g",
      },
    ],
    profiles: [],
    defaultProfileId: null,
    groups: [
      { id: "g", displayName: "Same" },
      { id: "other", displayName: "Same" },
    ],
  };
  vi.mocked(invoke).mockResolvedValueOnce(value);
  expect(await getManual("game")).toEqual(value);
  vi.mocked(invoke).mockResolvedValueOnce(null);
  await mutateManual(value, {
    kind: "assignGroup",
    installationId: "i",
    groupId: null,
  });
  expect(invoke).toHaveBeenLastCalledWith("manual_mutate", {
    request: {
      gameId: "game",
      expectedRevision: 7,
      action: { kind: "assignGroup", installationId: "i", groupId: null },
    },
  });
  for (const bad of [
    { ...value, groups: [] },
    {
      ...value,
      groups: Array.from({ length: 101 }, (_, i) => ({
        id: String(i),
        displayName: "Name",
      })),
    },
    { ...value, groups: [value.groups[0], value.groups[0]] },
  ]) {
    vi.mocked(invoke).mockResolvedValueOnce(bad);
    await expect(getManual("game")).rejects.toMatchObject({
      code: "INVALID_RESPONSE",
    });
  }
});
