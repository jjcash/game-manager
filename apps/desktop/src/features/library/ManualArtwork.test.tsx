// SPDX-License-Identifier: GPL-3.0-or-later
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ManualEditor } from "./ManualEditor";
import { getManual, type Details } from "../../ipc/manual";
import { changeArtwork } from "../../ipc/artwork";
import { artworkQueue, type ArtworkResult } from "./artwork";
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

const ready: ArtworkResult = {
  status: "ready",
  dataUrl: "data:image/png;base64,AAAA",
  cache: "miss",
  workMs: 1,
  decodeMs: 1,
};
const failed = { ...ready, status: "unavailable", dataUrl: null };
const unavailable = "Assigned artwork is unavailable.";
const artworkStatus = () =>
  within(screen.getByRole("region", { name: "Local artwork" })).getByRole(
    "status",
  );
function observeArtwork() {
  const listeners: ((result: ArtworkResult) => void)[] = [];
  vi.spyOn(artworkQueue, "subscribe").mockImplementation(
    (_id, _priority, listener) => {
      listeners.push(listener);
      return vi.fn();
    },
  );
  return {
    listeners,
    async waitForSubscriptions(count: number) {
      // The preview DOM can commit before Thumbnail's subscription effect runs.
      await waitFor(() => expect(listeners).toHaveLength(count));
    },
  };
}
it("announces a confirmed assigned-artwork failure once, with safe recovery wording", async () => {
  const { listeners, waitForSubscriptions } = observeArtwork();
  vi.mocked(getManual).mockResolvedValue({
    ...details,
    artworkAssetId: "asset",
  });
  render(<ManualEditor gameId="canonical" changed={vi.fn()} close={vi.fn()} />);
  await screen.findByRole("region", { name: "Local artwork" });
  await waitForSubscriptions(1);
  const status = artworkStatus();
  expect(status.textContent).toBe("");
  const mutations = vi.fn();
  const observer = new MutationObserver(mutations);
  observer.observe(status, {
    childList: true,
    subtree: true,
    characterData: true,
  });
  act(() => listeners[0](failed));
  await screen.findByText(/Assigned artwork is unavailable/);
  expect(status.textContent).toBe(
    "Assigned artwork is unavailable. Replace it with another image, or restore the preserved original to Game Manager's artwork storage from your separate backup.",
  );
  expect(status.getAttribute("aria-atomic")).toBe("true");
  await waitFor(() => expect(mutations).toHaveBeenCalledTimes(1));
  act(() => listeners[0]({ ...failed }));
  await Promise.resolve();
  expect(mutations).toHaveBeenCalledTimes(1);
  observer.disconnect();
});
it("keeps unassigned, pending, deferred and successfully decoded artwork silent", async () => {
  const { listeners, waitForSubscriptions } = observeArtwork();
  const editor = render(
    <ManualEditor gameId="canonical" changed={vi.fn()} close={vi.fn()} />,
  );
  await screen.findByRole("region", { name: "Local artwork" });
  await waitForSubscriptions(1);
  act(() => listeners[0](failed));
  expect(screen.queryByText(new RegExp(unavailable))).toBeNull();
  editor.unmount();
  vi.mocked(getManual).mockResolvedValue({
    ...details,
    artworkAssetId: "asset",
  });
  const assigned = render(
    <ManualEditor gameId="canonical" changed={vi.fn()} close={vi.fn()} />,
  );
  await screen.findByRole("region", { name: "Local artwork" });
  await waitForSubscriptions(2);
  for (const status of ["missing", "busy", "stopping", "deferred"]) {
    act(() => listeners[1]({ ...failed, status }));
    expect(artworkStatus().textContent).toBe("");
  }
  act(() => listeners[1](ready));
  expect(artworkStatus().textContent).toBe("");
  fireEvent.load(assigned.container.querySelector("img")!);
  expect(artworkStatus().textContent).toBe("");
  expect(
    assigned.container.querySelector(".thumbnail")?.getAttribute("aria-hidden"),
  ).toBe("true");
  expect(screen.queryByRole("img")).toBeNull();
});
it("explains browser image errors and recovers on replace, ignoring released failures", async () => {
  const { listeners, waitForSubscriptions } = observeArtwork();
  vi.mocked(getManual).mockResolvedValue({ ...details, artworkAssetId: "old" });
  vi.mocked(changeArtwork).mockResolvedValue(true);
  const editor = render(
    <ManualEditor gameId="canonical" changed={vi.fn()} close={vi.fn()} />,
  );
  const replace = await screen.findByRole("button", {
    name: "Replace local artwork…",
  });
  await waitForSubscriptions(1);
  act(() => listeners[0](ready));
  fireEvent.error(editor.container.querySelector("img")!);
  expect(artworkStatus().textContent).toContain(unavailable);
  vi.mocked(getManual).mockResolvedValueOnce({
    ...details,
    revision: 5,
    artworkAssetId: "new",
  });
  fireEvent.click(replace);
  await waitForSubscriptions(2);
  expect(artworkStatus().textContent).toBe("");
  act(() => listeners[0](failed));
  expect(artworkStatus().textContent).toBe("");
  act(() => listeners[1](ready));
  fireEvent.load(editor.container.querySelector("img")!);
  expect(artworkStatus().textContent).toBe("");
});
it("clears stale failures after clear and same-revision stale-request refresh", async () => {
  const { listeners, waitForSubscriptions } = observeArtwork();
  const assigned = { ...details, artworkAssetId: "asset" };
  vi.mocked(getManual).mockResolvedValue(assigned);
  vi.mocked(changeArtwork)
    .mockRejectedValueOnce({ code: "stale" })
    .mockResolvedValueOnce(true);
  render(<ManualEditor gameId="canonical" changed={vi.fn()} close={vi.fn()} />);
  const replace = await screen.findByRole("button", {
    name: "Replace local artwork…",
  });
  await waitForSubscriptions(1);
  act(() => listeners[0](failed));
  fireEvent.click(replace);
  await waitForSubscriptions(2);
  expect(artworkStatus().textContent).toBe("");
  act(() => listeners[0](failed));
  expect(artworkStatus().textContent).toBe("");
  act(() => listeners[1](failed));
  expect(artworkStatus().textContent).toContain(unavailable);
  vi.mocked(getManual).mockResolvedValueOnce({ ...details, revision: 5 });
  fireEvent.click(screen.getByRole("button", { name: "Clear artwork" }));
  await screen.findByRole("button", { name: "Choose local artwork…" });
  await waitForSubscriptions(3);
  act(() => listeners[1](failed));
  act(() => listeners[2]({ ...failed, status: "missing" }));
  expect(artworkStatus().textContent).toBe("");
});
it("suppresses a selected game's failure immediately while another game's details load", async () => {
  const { listeners, waitForSubscriptions } = observeArtwork();
  vi.mocked(getManual).mockResolvedValue({ ...details, artworkAssetId: "old" });
  const editor = render(
    <ManualEditor gameId="canonical" changed={vi.fn()} close={vi.fn()} />,
  );
  await screen.findByRole("region", { name: "Local artwork" });
  await waitForSubscriptions(1);
  act(() => listeners[0](failed));
  let resolve!: (value: Details) => void;
  vi.mocked(getManual).mockReturnValueOnce(
    new Promise((r) => {
      resolve = r;
    }),
  );
  editor.rerender(
    <ManualEditor gameId="other" changed={vi.fn()} close={vi.fn()} />,
  );
  expect(screen.queryByText(new RegExp(unavailable))).toBeNull();
  act(() => listeners[0](failed));
  await act(async () =>
    resolve({ ...details, id: "other", artworkAssetId: "new" }),
  );
  await waitForSubscriptions(2);
  expect(artworkStatus().textContent).toBe("");
  act(() => listeners[0](failed));
  expect(artworkStatus().textContent).toBe("");
});
it("retires a confirmed mutation's failure while bounded assignment details refresh", async () => {
  const { listeners, waitForSubscriptions } = observeArtwork();
  vi.mocked(getManual).mockResolvedValue({ ...details, artworkAssetId: "old" });
  vi.mocked(changeArtwork).mockResolvedValue(true);
  render(<ManualEditor gameId="canonical" changed={vi.fn()} close={vi.fn()} />);
  const replace = await screen.findByRole("button", {
    name: "Replace local artwork…",
  });
  await waitForSubscriptions(1);
  act(() => listeners[0](failed));
  let resolve!: (value: Details) => void;
  vi.mocked(getManual).mockReturnValueOnce(
    new Promise((r) => {
      resolve = r;
    }),
  );
  fireEvent.click(replace);
  await waitFor(() => expect(getManual).toHaveBeenCalledTimes(2));
  expect(screen.queryByText(new RegExp(unavailable))).toBeNull();
  act(() => listeners[0](failed));
  expect(screen.queryByText(new RegExp(unavailable))).toBeNull();
  await act(async () =>
    resolve({ ...details, revision: 5, artworkAssetId: "new" }),
  );
  await waitForSubscriptions(2);
  expect(artworkStatus().textContent).toBe("");
});
