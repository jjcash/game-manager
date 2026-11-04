// SPDX-License-Identifier: GPL-3.0-or-later
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { StandaloneLaunch } from "./StandaloneLaunch";
import type { Details } from "../../ipc/manual";
import { LaunchError, launchStandalone } from "../../ipc/standaloneLaunch";
vi.mock("../../ipc/standaloneLaunch", async (original) => ({
  ...(await original<typeof import("../../ipc/standaloneLaunch")>()),
  launchStandalone: vi.fn(),
}));
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
  entries: [
    {
      id: "e",
      sourceId: "s",
      sourceName: "Manual",
      sourceType: "manual",
      completionStatus: "not_started",
    },
  ],
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
  artworkAssetId: null,
};

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
it("requires explicit selection even for one profile; never invokes on navigation or default selection", () => {
  const { rerender } = render(
    <StandaloneLaunch details={details} refresh={vi.fn()} />,
  );
  expect(
    (screen.getByLabelText("Launch profile") as HTMLSelectElement).value,
  ).toBe("");
  expect(
    (screen.getByRole("button", { name: "Launch" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  rerender(
    <StandaloneLaunch
      details={{ ...details, revision: 5 }}
      refresh={vi.fn()}
    />,
  );
  expect(launchStandalone).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("Launch profile"), {
    target: { value: details.profiles[0].id },
  });
  expect(launchStandalone).not.toHaveBeenCalled();
  expect(
    (screen.getByRole("button", { name: "Launch" }) as HTMLButtonElement)
      .disabled,
  ).toBe(false);
});
it("preselects only an explicit supported default and launches only on a deliberate button action", async () => {
  vi.mocked(launchStandalone).mockResolvedValue({ pid: 42 });
  render(
    <StandaloneLaunch
      details={{ ...details, defaultProfileId: details.profiles[0].id }}
      refresh={vi.fn()}
    />,
  );
  expect(
    (screen.getByLabelText("Launch profile") as HTMLSelectElement).value,
  ).toBe(details.profiles[0].id);
  expect(launchStandalone).not.toHaveBeenCalled();
  const button = screen.getByRole("button", { name: "Launch" });
  button.focus();
  expect(document.activeElement).toBe(button);
  expect(button.getAttribute("aria-describedby")).toBe(
    "standalone-launch-help",
  );
  fireEvent.click(button);
  await screen.findByText("Launch request sent to Windows.");
  expect(launchStandalone).toHaveBeenCalledExactlyOnceWith(
    expect.objectContaining({ defaultProfileId: details.profiles[0].id }),
    details.profiles[0],
  );
});
it("disables repeated in-flight submissions then permits a new deliberate attempt", async () => {
  let resolve!: (value: { pid: number }) => void;
  vi.mocked(launchStandalone).mockImplementation(
    () =>
      new Promise((r) => {
        resolve = r;
      }),
  );
  render(
    <StandaloneLaunch
      details={{ ...details, defaultProfileId: details.profiles[0].id }}
      refresh={vi.fn()}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Launch" }));
  expect(
    (screen.getByRole("button", { name: "Launching…" }) as HTMLButtonElement)
      .disabled,
  ).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Launching…" }));
  expect(launchStandalone).toHaveBeenCalledTimes(1);
  resolve({ pid: 42 });
  await screen.findByRole("button", { name: "Launch" });
  fireEvent.click(screen.getByRole("button", { name: "Launch" }));
  expect(launchStandalone).toHaveBeenCalledTimes(2);
  resolve({ pid: 43 });
});
it("shows unsupported sources and targets without selecting unsupported defaults", () => {
  render(
    <StandaloneLaunch
      details={{
        ...details,
        defaultProfileId: details.profiles[0].id,
        entries: details.entries.map((e) => ({ ...e, sourceType: "steam" })),
      }}
      refresh={vi.fn()}
    />,
  );
  expect(screen.getByText(/Unsupported profile or source/)).toBeTruthy();
  expect(
    (screen.getByLabelText("Launch profile") as HTMLSelectElement).value,
  ).toBe("");
  expect(launchStandalone).not.toHaveBeenCalled();
});
it("refreshes stale context, clears selection and requires another deliberate action", async () => {
  const refresh = vi.fn().mockResolvedValue(undefined);
  vi.mocked(launchStandalone).mockRejectedValue(
    new LaunchError("staleContext", "Configuration changed."),
  );
  render(
    <StandaloneLaunch
      details={{ ...details, defaultProfileId: details.profiles[0].id }}
      refresh={refresh}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Launch" }));
  await waitFor(() => expect(refresh).toHaveBeenCalledTimes(1));
  expect(
    (screen.getByLabelText("Launch profile") as HTMLSelectElement).value,
  ).toBe("");
  expect(launchStandalone).toHaveBeenCalledTimes(1);
  expect(screen.getByRole("alert").textContent).toBe("Configuration changed.");
});
it("keeps errors concise and does not retry native creation failure", async () => {
  vi.mocked(launchStandalone).mockRejectedValue(
    new LaunchError("osFailure", "Windows could not create the process."),
  );
  render(
    <StandaloneLaunch
      details={{ ...details, defaultProfileId: details.profiles[0].id }}
      refresh={vi.fn()}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Launch" }));
  await screen.findByText("Windows could not create the process.");
  expect(launchStandalone).toHaveBeenCalledTimes(1);
  expect(screen.queryByText("Launch request sent to Windows.")).toBeNull();
});
