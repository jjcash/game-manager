// SPDX-License-Identifier: GPL-3.0-or-later
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { LaunchProfiles } from "./LaunchProfiles";
import type { Details } from "../../ipc/manual";
const details: Details = {
  id: "g",
  revision: 4,
  origin: "manual",
  metadata: {
    title: "x",
    releaseDate: null,
    genre: null,
    developer: null,
    favorite: false,
  },
  overriddenFields: [],
  entries: [],
  groups: [],
  installations: [
    {
      id: "i",
      entryId: "e",
      rootPathRef: "Z:\\synthetic",
      platform: "windows",
      version: null,
      detectionState: "unknown",
      stateOrigin: "user",
      groupId: null,
    },
  ],
  profiles: [
    {
      id: "a".repeat(32),
      gameId: "g",
      installationId: "i",
      revision: 7,
      provenance: "user",
      configuration: {
        displayName: "日本語",
        launcherKind: "standalone",
        executableRef: { kind: "installationRelative", value: "game.exe" },
        arguments: ["", "two words"],
        workingDirectoryRef: null,
      },
    },
  ],
  defaultProfileId: null,
};
afterEach(cleanup);
it("edits the immutable binding, retains failed drafts, duplicates and confirms removal", async () => {
  const save = vi.fn().mockResolvedValue(false);
  const confirm = vi.fn();
  render(<LaunchProfiles details={details} save={save} confirm={confirm} />);
  fireEvent.click(screen.getByText("Edit profile aaaaaaaa"));
  expect(
    (screen.getByLabelText("Profile installation") as HTMLSelectElement)
      .disabled,
  ).toBe(true);
  expect(
    (
      screen.getByLabelText(
        "Literal arguments (JSON array)",
      ) as HTMLTextAreaElement
    ).value,
  ).toBe('["","two words"]');
  fireEvent.change(screen.getByLabelText("Profile name"), {
    target: { value: "Draft 🎮" },
  });
  fireEvent.click(screen.getByText("Save profile configuration"));
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith({
      kind: "launchProfile",
      action: {
        operation: "save",
        profileId: "a".repeat(32),
        expectedProfileRevision: 7,
        configuration: {
          ...details.profiles[0].configuration,
          displayName: "Draft 🎮",
        },
      },
    }),
  );
  expect(
    (screen.getByLabelText("Profile name") as HTMLInputElement).value,
  ).toBe("Draft 🎮");
  fireEvent.click(screen.getByText("Duplicate profile aaaaaaaa"));
  expect(save.mock.lastCall?.[0].action).toEqual({
    operation: "duplicate",
    profileId: "a".repeat(32),
    expectedProfileRevision: 7,
  });
  fireEvent.click(screen.getByText("Remove profile aaaaaaaa"));
  expect(confirm).toHaveBeenCalled();
  expect(save).toHaveBeenCalledTimes(2);
});
it("validates JSON before writes and explicitly selects or clears defaults", () => {
  const save = vi.fn().mockResolvedValue(true);
  render(
    <LaunchProfiles
      details={{ ...details, defaultProfileId: "a".repeat(32) }}
      save={save}
      confirm={vi.fn()}
    />,
  );
  fireEvent.change(screen.getByLabelText("Profile installation"), {
    target: { value: "i" },
  });
  fireEvent.change(screen.getByLabelText("Profile name"), {
    target: { value: "New" },
  });
  fireEvent.change(screen.getByLabelText("Executable reference"), {
    target: { value: "game.exe" },
  });
  fireEvent.change(screen.getByLabelText("Literal arguments (JSON array)"), {
    target: { value: "[1]" },
  });
  fireEvent.click(screen.getByText("Create profile"));
  expect(save).not.toHaveBeenCalled();
  expect(screen.getByRole("alert").textContent).toContain("literal strings");
  fireEvent.click(screen.getByText("Clear default profile"));
  expect(save.mock.lastCall?.[0].action).toEqual({
    operation: "setDefault",
    profileId: null,
    expectedProfileRevision: null,
  });
  fireEvent.change(screen.getByLabelText("Default LaunchProfile"), {
    target: { value: "a".repeat(32) },
  });
  expect(save.mock.lastCall?.[0].action).toEqual({
    operation: "setDefault",
    profileId: "a".repeat(32),
    expectedProfileRevision: 7,
  });
});
