// SPDX-License-Identifier: GPL-3.0-or-later
import { expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { changeArtwork, artworkError } from "./artwork";
import type { Details } from "./manual";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
it("sends only game identity/revision, distinguishes cancellation and rejects malformed replies", async () => {
  const details = { id: "manual-id", revision: 7 } as Details;
  vi.mocked(invoke)
    .mockResolvedValueOnce(false)
    .mockResolvedValueOnce(true)
    .mockResolvedValueOnce(null)
    .mockResolvedValueOnce("bad");
  expect(await changeArtwork(details, false)).toBe(false);
  expect(await changeArtwork(details, false)).toBe(true);
  expect(invoke).toHaveBeenLastCalledWith("artwork_choose", {
    request: { gameId: "manual-id", expectedRevision: 7 },
  });
  expect(await changeArtwork(details, true)).toBe(true);
  expect(invoke).toHaveBeenLastCalledWith("artwork_clear", {
    request: { gameId: "manual-id", expectedRevision: 7 },
  });
  await expect(changeArtwork(details, false)).rejects.toEqual({
    code: "connection",
  });
  await expect(
    changeArtwork({ ...details, id: "../outside" }, false),
  ).rejects.toEqual({ code: "invalidRequest" });
});
it("never renders backend messages or paths", () => {
  expect(
    artworkError({ code: "storage", message: "C:\\private" }),
  ).not.toContain("private");
  expect(artworkError({ code: "stale" })).toContain("refreshed");
  expect(artworkError(new Error("C:\\private"))).toContain(
    "could not be confirmed",
  );
});
