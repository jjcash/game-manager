// SPDX-License-Identifier: GPL-3.0-or-later
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  getLibraryInfo,
  getLibraryWindow,
  LibraryError,
} from "../../ipc/library";
import { LibraryView } from "./LibraryView";
import { useLibraryUI } from "./state";
import { pageGate } from "./requestGate";
import { StrictMode } from "react";
vi.mock("../../ipc/library", async (original) => ({
  ...(await original<typeof import("../../ipc/library")>()),
  getLibraryInfo: vi.fn(),
  getLibraryWindow: vi.fn(),
}));
let total = 15000;
const row = (i: number) => ({
  id: `id-${i}`,
  title: `Game ${i}`,
  releaseDate: null,
  sourceNames: "Synthetic",
  entryCount: 2,
  installationCount: 2,
  installed: true,
});
const response = (offset: number) => ({
  offset,
  items: Array.from({ length: Math.min(100, total - offset) }, (_, i) =>
    row(offset + i),
  ),
  queryMs: 1,
});
beforeEach(() => {
  total = 15000;
  useLibraryUI.getState().reset();
  vi.mocked(getLibraryInfo).mockResolvedValue({
    total,
    source: "Synthetic",
    diagnostics: false,
  });
  vi.mocked(getLibraryWindow).mockImplementation(async (offset) =>
    response(offset),
  );
  vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockReturnValue(380);
  vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockReturnValue(900);
  vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockReturnValue(380);
  vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockImplementation(
    function (this: HTMLElement) {
      return parseFloat(
        (this.firstElementChild as HTMLElement)?.style.height || "380",
      );
    },
  );
  HTMLElement.prototype.scrollTo = function (
    options?: ScrollToOptions | number,
  ) {
    if (typeof options === "object")
      this.scrollTop = options.top ?? this.scrollTop;
    this.dispatchEvent(new Event("scroll"));
  };
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  );
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});
describe("Details View", () => {
  it("survives StrictMode cleanup and bounds rapid-scroll requests", async () => {
    let finish!: (value: ReturnType<typeof response>) => void;
    vi.mocked(getLibraryWindow).mockImplementationOnce(
      () =>
        new Promise((r) => {
          finish = r;
        }),
    );
    render(
      <StrictMode>
        <LibraryView />
      </StrictMode>,
    );
    await waitFor(() => expect(getLibraryWindow).toHaveBeenCalledTimes(1));
    const list = screen.getByRole("rowgroup");
    for (let i = 1; i < 100; i++)
      fireEvent.scroll(list, { target: { scrollTop: i * 5000 } });
    expect(pageGate.queued).toBeLessThanOrEqual(2);
    expect(getLibraryWindow).toHaveBeenCalledTimes(1);
    finish(response(0));
    await screen.findByText("Game 13027");
    expect(getLibraryWindow).toHaveBeenCalledTimes(2);
    expect(screen.getAllByRole("row").length).toBeLessThanOrEqual(28);
  });

  it("ignores out-of-order metadata and cancels queued work on unmount", async () => {
    let oldInfo!: (value: Awaited<ReturnType<typeof getLibraryInfo>>) => void;
    vi.mocked(getLibraryInfo).mockImplementationOnce(
      () =>
        new Promise((r) => {
          oldInfo = r;
        }),
    );
    const view = render(<LibraryView />);
    fireEvent.click(screen.getByRole("button", { name: "Refresh library" }));
    await screen.findByText("Game 0");
    oldInfo({ total: 0, source: "Stale metadata", diagnostics: false });
    await waitFor(() =>
      expect(screen.getByRole("grid").getAttribute("aria-rowcount")).toBe(
        "15001",
      ),
    );
    let finish!: (value: ReturnType<typeof response>) => void;
    vi.mocked(getLibraryWindow).mockImplementationOnce(
      () =>
        new Promise((r) => {
          finish = r;
        }),
    );
    fireEvent.scroll(screen.getByRole("rowgroup"), {
      target: { scrollTop: 38000 },
    });
    await waitFor(() => expect(getLibraryWindow).toHaveBeenCalledTimes(2));
    fireEvent.scroll(screen.getByRole("rowgroup"), {
      target: { scrollTop: 76000 },
    });
    view.unmount();
    expect(pageGate.queued).toBe(0);
    finish(response(1000));
    await Promise.resolve();
    expect(getLibraryWindow).toHaveBeenCalledTimes(2);
  });
  it("mounts only the viewport plus overscan and navigates to the final canonical game", async () => {
    render(<LibraryView />);
    await screen.findByText("Game 0");
    expect(screen.getAllByRole("row").length).toBeLessThanOrEqual(28);
    fireEvent.click(screen.getByText("Game 0"));
    expect(useLibraryUI.getState().selection?.id).toBe("id-0");
    const list = screen.getByRole("rowgroup");
    fireEvent.keyDown(list, { key: "End" });
    await screen.findByText("Game 14999");
    await waitFor(() =>
      expect(useLibraryUI.getState().selection?.id).toBe("id-14999"),
    );
    expect(screen.getAllByRole("row").length).toBeLessThanOrEqual(28);
    fireEvent.keyDown(list, { key: "ArrowUp" });
    expect(useLibraryUI.getState().selection?.id).toBe("id-14998");
    fireEvent.keyDown(list, { key: "Home" });
    await screen.findByText("Game 0");
    expect(getLibraryWindow).toHaveBeenCalledWith(14900, 100);
  });
  it("shows initial loading, partial error and retry without reporting an empty library", async () => {
    let finish!: (value: Awaited<ReturnType<typeof getLibraryInfo>>) => void;
    vi.mocked(getLibraryInfo).mockImplementationOnce(
      () =>
        new Promise((r) => {
          finish = r;
        }),
    );
    vi.mocked(getLibraryWindow).mockRejectedValueOnce(
      new LibraryError("DATABASE", "Page failed"),
    );
    render(<LibraryView />);
    expect(screen.getByText("Loading library…")).toBeTruthy();
    finish({ total, source: "Synthetic", diagnostics: false });
    await screen.findByRole("alert");
    expect(screen.queryByText(/Your library is empty/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Retry library" }));
    await screen.findByText("Game 0");
  });
  it("renders empty and persistent initialization failures distinctly", async () => {
    vi.mocked(getLibraryInfo).mockResolvedValueOnce({
      total: 0,
      source: "Empty",
      diagnostics: false,
    });
    const view = render(<LibraryView />);
    await screen.findByText(/Your library is empty/);
    vi.mocked(getLibraryInfo).mockRejectedValueOnce(
      new LibraryError("MIGRATION", "Database preserved"),
    );
    fireEvent.click(screen.getByRole("button", { name: "Refresh library" }));
    await screen.findByRole("alert");
    expect(screen.queryByText(/Your library is empty/)).toBeNull();
    view.unmount();
  });
  it("discards late old page results across refresh and dataset change", async () => {
    let finish!: (v: ReturnType<typeof response>) => void;
    vi.mocked(getLibraryWindow).mockImplementationOnce(
      () =>
        new Promise((r) => {
          finish = r;
        }),
    );
    render(<LibraryView />);
    await waitFor(() => expect(getLibraryWindow).toHaveBeenCalled());
    vi.mocked(getLibraryInfo).mockResolvedValue({
      total: 500,
      source: "New dataset",
      diagnostics: false,
    });
    total = 500;
    fireEvent.click(screen.getByRole("button", { name: "Refresh library" }));
    finish({
      offset: 0,
      items: [{ ...row(0), title: "Stale old game" }],
      queryMs: 1,
    });
    await screen.findByText("Game 0");
    expect(screen.queryByText("Stale old game")).toBeNull();
    expect(screen.getByRole("grid").getAttribute("aria-rowcount")).toBe("501");
  });
  it("cleans up on navigation and restores stable selection on return", async () => {
    const view = render(<LibraryView />);
    await screen.findByText("Game 0");
    fireEvent.click(screen.getByText("Game 0"));
    fireEvent.scroll(screen.getByRole("rowgroup"), {
      target: { scrollTop: 38000 },
    });
    await screen.findByText("Game 1000");
    expect(useLibraryUI.getState().selection?.id).toBe("id-0");
    // The DOM can move before the browser dispatches its next scroll event.
    screen.getByRole("rowgroup").scrollTop = 38100;
    view.unmount();
    expect(useLibraryUI.getState().scrollOffset).toBe(38100);
    render(<LibraryView />);
    await screen.findByText("Game 1000");
    expect(useLibraryUI.getState().selection?.id).toBe("id-0");
  });
});
