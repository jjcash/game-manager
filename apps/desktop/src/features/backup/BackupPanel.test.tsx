// SPDX-License-Identifier: GPL-3.0-or-later
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { BackupPanel } from "./BackupPanel";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const snapshot = {
  formatVersion: 1,
  applicationVersion: "0.1.0",
  schemaVersion: 2,
  bytes: 65536,
  counts: { games: 3, librarySources: 2, libraryEntries: 4, installations: 3 },
};
const status = {
  available: true,
  restartRequired: false,
  outcome: null,
  candidate: null,
};
beforeEach(() => {
  vi.mocked(invoke).mockReset();
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "backup_get_status") return status;
    return undefined;
  });
  HTMLDialogElement.prototype.showModal = function () {
    this.setAttribute("open", "");
  };
  HTMLDialogElement.prototype.close = function () {
    this.removeAttribute("open");
  };
});
afterEach(cleanup);
it("creates verified backup and handles native-dialog cancellation without any path IPC", async () => {
  vi.mocked(invoke).mockImplementation(async (command) =>
    command === "backup_get_status" ? status : snapshot,
  );
  render(<BackupPanel />);
  const create = await screen.findByRole("button", { name: "Create backup…" });
  await waitFor(() => expect(create.hasAttribute("disabled")).toBe(false));
  fireEvent.click(create);
  expect(await screen.findByText(/Backup created and verified/)).toBeTruthy();
  expect(invoke).toHaveBeenCalledWith("backup_create");
  vi.mocked(invoke).mockImplementation(async (command) =>
    command === "backup_get_status" ? status : null,
  );
  fireEvent.click(create);
  expect(await screen.findByText(/No backup created/)).toBeTruthy();
});
it("does not enable confirmation for a rejected backup", async () => {
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "backup_get_status") return status;
    throw {
      code: "WRONG_PURPOSE",
      message: "Synthetic benchmark databases cannot be restored.",
    };
  });
  render(<BackupPanel />);
  const select = await screen.findByRole("button", {
    name: "Select backup to restore…",
  });
  await waitFor(() => expect(select.hasAttribute("disabled")).toBe(false));
  fireEvent.click(select);
  expect(await screen.findByRole("alert")).toHaveProperty(
    "textContent",
    "Synthetic benchmark databases cannot be restored.",
  );
  expect(
    screen.queryByRole("button", { name: "Review replacement…" }),
  ).toBeNull();
  expect(invoke).not.toHaveBeenCalledWith(
    "backup_confirm_restore",
    expect.anything(),
  );
});
it("validates before confirmation, explains replacement, traps Tab and cancels without confirming", async () => {
  let selected = false;
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "backup_get_status")
      return { ...status, candidate: selected ? { token: 7, snapshot } : null };
    if (command === "backup_select_restore") {
      selected = true;
      return { token: 7, snapshot };
    }
    if (command === "backup_cancel_restore") {
      selected = false;
      return;
    }
  });
  render(<BackupPanel />);
  const select = await screen.findByRole("button", {
    name: "Select backup to restore…",
  });
  await waitFor(() => expect(select.hasAttribute("disabled")).toBe(false));
  fireEvent.click(select);
  fireEvent.click(
    await screen.findByRole("button", { name: "Review replacement…" }),
  );
  const dialog = await screen.findByRole("dialog");
  expect(
    screen.getByText(/older backup has no installation groups/),
  ).toBeTruthy();
  expect(dialog.textContent).toContain("Records are not merged");
  expect(dialog.textContent).toContain("Installation groups are replaced too");
  expect(dialog.textContent).toContain("verified copy of your current library");
  const confirm = screen.getByRole("button", {
    name: "Confirm library replacement",
  });
  confirm.focus();
  fireEvent.keyDown(dialog, { key: "Tab" });
  expect(document.activeElement).toBe(
    screen.getByRole("button", { name: "Cancel restore" }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Cancel restore" }));
  expect(await screen.findByText(/Restore cancelled/)).toBeTruthy();
  expect(invoke).not.toHaveBeenCalledWith(
    "backup_confirm_restore",
    expect.anything(),
  );
  expect(screen.queryByRole("dialog")).toBeNull();
});
it("uses exact validated token, closes for restart and reports verified outcome separately", async () => {
  let confirmed = false;
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "backup_get_status")
      return {
        ...status,
        restartRequired: confirmed,
        candidate: confirmed ? null : { token: 9, snapshot },
      };
    if (command === "backup_confirm_restore") {
      confirmed = true;
      return;
    }
  });
  render(<BackupPanel />);
  fireEvent.click(
    await screen.findByRole("button", { name: "Review replacement…" }),
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Confirm library replacement" }),
  );
  const restart = await screen.findByRole("button", {
    name: "Restart and finish",
  });
  expect(invoke).toHaveBeenCalledWith("backup_confirm_restore", { token: 9 });
  expect(screen.queryByText(/Restore completed and verified/)).toBeNull();
  expect(
    screen
      .getByRole("button", { name: "Create backup…" })
      .hasAttribute("disabled"),
  ).toBe(true);
  fireEvent.click(restart);
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("backup_restart"));
});
it("preserves explicit recovery guidance after preparation failure", async () => {
  let attempted = false;
  vi.mocked(invoke).mockImplementation(async (command) => {
    if (command === "backup_get_status")
      return {
        ...status,
        restartRequired: attempted,
        candidate: attempted ? null : { token: 2, snapshot },
      };
    if (command === "backup_confirm_restore") {
      attempted = true;
      throw {
        message:
          "Could not preserve recovery copy. Current library is unchanged.",
      };
    }
  });
  render(<BackupPanel />);
  fireEvent.click(
    await screen.findByRole("button", { name: "Review replacement…" }),
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Confirm library replacement" }),
  );
  expect(await screen.findByRole("alert")).toHaveProperty(
    "textContent",
    "Could not preserve recovery copy. Current library is unchanged.",
  );
  expect(
    await screen.findByRole("button", { name: "Restart and finish" }),
  ).toBeTruthy();
});
it("shows recovery and excludes benchmark maintenance", async () => {
  vi.mocked(invoke).mockResolvedValue({
    ...status,
    outcome: "recovered",
    available: false,
  });
  render(<BackupPanel />);
  expect(
    await screen.findByText(
      /preserved prior library was recovered and verified/,
    ),
  ).toBeTruthy();
  expect(
    screen
      .getByRole("button", { name: "Create backup…" })
      .hasAttribute("disabled"),
  ).toBe(true);
});
