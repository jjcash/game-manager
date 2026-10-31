// SPDX-License-Identifier: GPL-3.0-or-later
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ManualEditor } from "./ManualEditor";
import { getManual, type Details } from "../../ipc/manual";
import { changeArtwork } from "../../ipc/artwork";
import { artworkQueue } from "./artwork";
vi.mock("../../ipc/manual", async (original) => ({
  ...(await original<typeof import("../../ipc/manual")>()),
  getManual: vi.fn(),
}));
vi.mock("../../ipc/artwork", async (original) => ({
  ...(await original<typeof import("../../ipc/artwork")>()),
  changeArtwork: vi.fn(),
}));
const details: Details = {
  id: "canonical",
  revision: 4,
  origin: "manual",
  metadata: {
    title: "Existing",
    releaseDate: null,
    genre: null,
    developer: null,
    favorite: false,
  },
  overriddenFields: [],
  entries: [],
  installations: [],
  groups: [],
  profiles: [],
  defaultProfileId: null,
  artworkAssetId: null,
};
beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function () {
    this.open = false;
  };
  vi.mocked(getManual).mockResolvedValue(structuredClone(details));
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
  vi.restoreAllMocks();
});
it("cancel/failure preserve artwork and drafts; replace/clear refresh revisions", async () => {
  const changed = vi.fn();
  const clear = vi.spyOn(artworkQueue, "clear");
  vi.mocked(changeArtwork)
    .mockResolvedValueOnce(false)
    .mockRejectedValueOnce({ code: "corrupt" })
    .mockResolvedValueOnce(true)
    .mockResolvedValueOnce(true);
  render(<ManualEditor gameId="canonical" changed={changed} close={vi.fn()} />);
  const choose = await screen.findByRole("button", {
    name: "Choose local artwork…",
  });
  fireEvent.change(screen.getByLabelText("Title"), {
    target: { value: "Unsaved draft" },
  });
  fireEvent.click(choose);
  await waitFor(() => expect(changeArtwork).toHaveBeenCalledTimes(1));
  await waitFor(() =>
    expect((choose.closest("fieldset") as HTMLFieldSetElement).disabled).toBe(
      false,
    ),
  );
  expect(changed).not.toHaveBeenCalled();
  fireEvent.click(choose);
  await screen.findByRole("alert");
  expect(changed).not.toHaveBeenCalled();
  vi.mocked(getManual).mockResolvedValueOnce({
    ...details,
    revision: 5,
    artworkAssetId: "a".repeat(32),
  });
  fireEvent.click(choose);
  await screen.findByRole("button", { name: "Replace local artwork…" });
  expect(clear).toHaveBeenCalled();
  expect(changed).toHaveBeenCalledTimes(1);
  expect((screen.getByLabelText("Title") as HTMLInputElement).value).toBe(
    "Unsaved draft",
  );
  vi.mocked(getManual).mockResolvedValueOnce({ ...details, revision: 6 });
  fireEvent.click(screen.getByRole("button", { name: "Clear artwork" }));
  await screen.findByRole("button", { name: "Choose local artwork…" });
  expect(changeArtwork).toHaveBeenLastCalledWith(
    expect.objectContaining({ revision: 5 }),
    true,
  );
  expect(changed).toHaveBeenCalledTimes(2);
});
it("stale requests refresh bounded details, retain drafts and require another action", async () => {
  const changed = vi.fn();
  vi.mocked(changeArtwork).mockRejectedValueOnce({ code: "stale" });
  render(<ManualEditor gameId="canonical" changed={changed} close={vi.fn()} />);
  const choose = await screen.findByRole("button", {
    name: "Choose local artwork…",
  });
  fireEvent.change(screen.getByLabelText("Title"), {
    target: { value: "Keep this" },
  });
  vi.mocked(getManual).mockResolvedValueOnce({ ...details, revision: 8 });
  fireEvent.click(choose);
  await waitFor(() => expect(getManual).toHaveBeenCalledTimes(2));
  expect((screen.getByLabelText("Title") as HTMLInputElement).value).toBe(
    "Keep this",
  );
  expect(changeArtwork).toHaveBeenCalledTimes(1);
  expect(changed).toHaveBeenCalledOnce();
});
it("hides stale assignment controls and preview if a successful save cannot refresh details", async () => {
  vi.mocked(changeArtwork).mockResolvedValueOnce(true);
  render(<ManualEditor gameId="canonical" changed={vi.fn()} close={vi.fn()} />);
  const choose = await screen.findByRole("button", {
    name: "Choose local artwork…",
  });
  vi.mocked(getManual).mockRejectedValueOnce(new Error("unavailable"));
  fireEvent.click(choose);
  await screen.findByText(
    "Artwork changed, but details could not refresh. Close and reopen the editor.",
  );
  expect(screen.queryByRole("region", { name: "Local artwork" })).toBeNull();
  expect(
    screen.queryByRole("button", { name: "Replace local artwork…" }),
  ).toBeNull();
});
