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
import {
  createManual,
  getManual,
  mutateManual,
  type Details,
} from "../../ipc/manual";
vi.mock("../../ipc/manual", async (original) => ({
  ...(await original<typeof import("../../ipc/manual")>()),
  createManual: vi.fn(),
  getManual: vi.fn(),
  mutateManual: vi.fn(),
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
  overriddenFields: ["title"],
  entries: [
    {
      id: "membership",
      sourceId: "manual",
      sourceName: "Manual",
      sourceType: "manual",
      completionStatus: "not_started",
    },
  ],
  installations: [],
  profiles: [],
  defaultProfileId: null,
  artworkAssetId: null,
  groups: [],
};
beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function () {
    this.open = false;
  };
  vi.mocked(getManual).mockResolvedValue(structuredClone(details));
  vi.mocked(createManual).mockResolvedValue("created");
  vi.mocked(mutateManual).mockResolvedValue(null);
});
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
it("keeps Tab and Shift+Tab inside the modal at both control boundaries", () => {
  render(<ManualEditor gameId={null} changed={vi.fn()} close={vi.fn()} />);
  const title = screen.getByLabelText("Title");
  const close = screen.getByRole("button", { name: "Close editor" });
  title.focus();
  fireEvent.keyDown(title, { key: "Tab", shiftKey: true });
  expect(document.activeElement).toBe(close);
  fireEvent.keyDown(close, { key: "Tab" });
  expect(document.activeElement).toBe(title);
  fireEvent.keyDown(title, { key: "Escape" });
  expect(document.activeElement).toBe(title);
});
it("creates a game with optional installation and preserves failed input", async () => {
  const changed = vi.fn(),
    close = vi.fn();
  vi.mocked(createManual).mockRejectedValueOnce(
    new Error("Database busy; retry"),
  );
  render(<ManualEditor gameId={null} changed={changed} close={close} />);
  fireEvent.change(screen.getByLabelText("Title"), {
    target: { value: "日本語 %_ '" },
  });
  fireEvent.click(screen.getByLabelText("Record an installation"));
  fireEvent.change(
    screen.getByLabelText("Installation location (metadata only)"),
    { target: { value: "Z:\\unaccessed" } },
  );
  fireEvent.click(screen.getByText("Create game"));
  await screen.findByText("Database busy; retry");
  expect((screen.getByLabelText("Title") as HTMLInputElement).value).toBe(
    "日本語 %_ '",
  );
  expect(changed).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("Create game"));
  await waitFor(() => expect(changed).toHaveBeenCalledOnce());
  expect(createManual).toHaveBeenLastCalledWith(
    expect.objectContaining({ title: "日本語 %_ '" }),
    "Manual",
    expect.objectContaining({
      rootPathRef: "Z:\\unaccessed",
      detectionState: "unknown",
    }),
  );
  expect(close).toHaveBeenCalledOnce();
});
it("saves explicit metadata, retains conflict drafts and requires canonical-removal confirmation", async () => {
  const changed = vi.fn();
  render(<ManualEditor gameId="canonical" changed={changed} close={vi.fn()} />);
  await screen.findByText("Save metadata");
  await waitFor(() =>
    expect((screen.getByLabelText("Title") as HTMLInputElement).value).toBe(
      "Existing",
    ),
  );
  fireEvent.change(screen.getByLabelText("Title"), {
    target: { value: "User override" },
  });
  vi.mocked(mutateManual).mockRejectedValueOnce(
    new Error("Conflict: reopen editor"),
  );
  fireEvent.click(screen.getByText("Save metadata"));
  await screen.findByText("Conflict: reopen editor");
  expect((screen.getByLabelText("Title") as HTMLInputElement).value).toBe(
    "User override",
  );
  expect(changed).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("Save metadata"));
  await waitFor(() => expect(changed).toHaveBeenCalledOnce());
  expect(mutateManual).toHaveBeenCalledWith(
    expect.objectContaining({ id: "canonical", revision: 4 }),
    expect.objectContaining({
      kind: "metadata",
      metadata: expect.objectContaining({ title: "User override" }),
    }),
  );
  fireEvent.click(screen.getByText("Remove manual game"));
  expect(screen.getByRole("alertdialog").textContent).toContain(
    "all 1 manual memberships",
  );
  expect(mutateManual).toHaveBeenCalledTimes(2);
  fireEvent.click(screen.getByText("Keep records"));
  expect(screen.queryByRole("alertdialog")).toBeNull();
});
it("keeps the last membership, supports multiple installations and removes metadata explicitly", async () => {
  vi.mocked(getManual).mockResolvedValue({
    ...details,
    installations: [
      {
        id: "copy",
        entryId: "membership",
        rootPathRef: "never-accessed",
        platform: "windows",
        version: null,
        detectionState: "present",
        stateOrigin: "user",
        groupId: null,
      },
    ],
  });
  render(<ManualEditor gameId="canonical" changed={vi.fn()} close={vi.fn()} />);
  await screen.findByText("Edit installation");
  expect(
    (screen.getByText("Remove membership") as HTMLButtonElement).disabled,
  ).toBe(true);
  fireEvent.click(screen.getByText("Edit installation"));
  expect(
    (screen.getByLabelText("Recorded presence") as HTMLSelectElement).value,
  ).toBe("present");
  fireEvent.change(screen.getByLabelText("Recorded presence"), {
    target: { value: "missing" },
  });
  fireEvent.click(screen.getByText("Save installation record"));
  await waitFor(() =>
    expect(mutateManual).toHaveBeenCalledWith(
      expect.anything(),
      expect.objectContaining({
        kind: "saveInstallation",
        installationId: "copy",
        installation: expect.objectContaining({ detectionState: "missing" }),
      }),
    ),
  );
  await waitFor(() =>
    expect(
      (screen.getByText("Remove installation record") as HTMLButtonElement)
        .disabled,
    ).toBe(false),
  );
  fireEvent.click(screen.getByText("Remove installation record"));
  expect(screen.getByRole("alertdialog").textContent).toContain(
    "Files remain untouched",
  );
  fireEvent.click(screen.getByText("Confirm removal"));
  await waitFor(() =>
    expect(mutateManual).toHaveBeenCalledWith(expect.anything(), {
      kind: "removeInstallation",
      installationId: "copy",
    }),
  );
});
it("shows details failures and leaves provider records read-only", async () => {
  vi.mocked(getManual).mockRejectedValueOnce(
    new Error("Record no longer exists"),
  );
  const r = render(
    <ManualEditor gameId="gone" changed={vi.fn()} close={vi.fn()} />,
  );
  await screen.findByRole("alert");
  expect(screen.getByText("Save metadata").matches(":disabled")).toBe(true);
  r.unmount();
  vi.mocked(getManual).mockResolvedValue({
    ...details,
    origin: "legacy",
    entries: [{ ...details.entries[0], sourceType: "provider" }],
  });
  render(<ManualEditor gameId="canonical" changed={vi.fn()} close={vi.fn()} />);
  await screen.findByText("Provider membership is read-only");
  expect(screen.queryByText("Remove manual game")).toBeNull();
});

it("organizes installations with explicit save, preserves conflict drafts and confirms ungrouping", async () => {
  const d: Details = {
    ...details,
    groups: [
      { id: "group-a", displayName: "日本語" },
      { id: "group-b", displayName: "日本語" },
    ],
    installations: [
      {
        id: "copy",
        entryId: "membership",
        rootPathRef: "metadata://one",
        platform: "windows",
        version: null,
        detectionState: "unknown",
        stateOrigin: "legacy",
        groupId: null,
      },
    ],
  };
  vi.mocked(getManual).mockResolvedValue(d);
  render(<ManualEditor gameId="canonical" changed={vi.fn()} close={vi.fn()} />);
  await screen.findByText(/Organize installation groups/);
  fireEvent.click(screen.getByText(/Organize installation groups/));
  fireEvent.change(screen.getByLabelText("Group name"), {
    target: { value: "Unsaved 🎮" },
  });
  expect(mutateManual).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("Cancel group edit"));
  expect((screen.getByLabelText("Group name") as HTMLInputElement).value).toBe(
    "",
  );
  fireEvent.change(screen.getByLabelText("Group to edit"), {
    target: { value: "group-b" },
  });
  fireEvent.change(screen.getByLabelText("Group name"), {
    target: { value: "Renamed 🎮" },
  });
  vi.mocked(mutateManual).mockRejectedValueOnce(
    new Error("Stale revision; reopen editor"),
  );
  fireEvent.click(screen.getByText("Save group name"));
  await screen.findByText("Stale revision; reopen editor");
  expect((screen.getByLabelText("Group name") as HTMLInputElement).value).toBe(
    "Renamed 🎮",
  );
  expect(mutateManual).toHaveBeenLastCalledWith(
    expect.objectContaining({ revision: 4 }),
    { kind: "saveGroup", groupId: "group-b", displayName: "Renamed 🎮" },
  );
  fireEvent.change(screen.getByLabelText("Installation to organize"), {
    target: { value: "copy" },
  });
  fireEvent.change(screen.getByLabelText("Assigned group"), {
    target: { value: "group-a" },
  });
  expect(mutateManual).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByText("Save group assignment"));
  await waitFor(() =>
    expect(mutateManual).toHaveBeenLastCalledWith(expect.anything(), {
      kind: "assignGroup",
      installationId: "copy",
      groupId: "group-a",
    }),
  );
  await waitFor(() =>
    expect(screen.getByText("Remove group").matches(":disabled")).toBe(false),
  );
  fireEvent.click(screen.getByText("Remove group"));
  expect(screen.getByRole("alertdialog").textContent).toContain(
    "become ungrouped",
  );
  expect(document.activeElement).toBe(screen.getByText("Keep records"));
  fireEvent.keyDown(screen.getByText("Keep records"), { key: "Tab" });
  expect(document.activeElement).toBe(screen.getByText("Confirm removal"));
  fireEvent.click(screen.getByText("Keep records"));
  expect(screen.queryByRole("alertdialog")).toBeNull();
  expect(mutateManual).toHaveBeenCalledTimes(2);
});
it("rejects a late bounded-details response after the editor unmounts", async () => {
  let resolve!: (value: Details) => void;
  vi.mocked(getManual).mockReturnValueOnce(
    new Promise((r) => {
      resolve = r;
    }),
  );
  const changed = vi.fn();
  const editor = render(
    <ManualEditor gameId="canonical" changed={changed} close={vi.fn()} />,
  );
  await waitFor(() => expect(getManual).toHaveBeenCalled());
  editor.unmount();
  resolve(details);
  await Promise.resolve();
  expect(changed).not.toHaveBeenCalled();
  expect(screen.queryByRole("dialog")).toBeNull();
});
