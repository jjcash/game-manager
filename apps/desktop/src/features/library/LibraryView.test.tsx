// SPDX-License-Identifier: GPL-3.0-or-later
import {
  cleanup,
  act,
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
  defaultCriteria,
} from "../../ipc/library";
import { LibraryView } from "./LibraryView";
import { useLibraryUI } from "./state";
import { pageGate } from "./requestGate";
import { artworkQueue, type ArtworkResult } from "./artwork";
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
  useLibraryUI.getState().setCriteria({ ...defaultCriteria });
  useLibraryUI.getState().reset();
  vi.mocked(getLibraryInfo).mockResolvedValue({
    total,
    libraryTotal: total,
    sources: [],
    queryMs: 0,
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
  vi.useRealTimers();
  vi.restoreAllMocks();
  vi.resetAllMocks();
  vi.unstubAllGlobals();
});
describe("Details View", () => {
  it("keeps filters, keyboard selection and bounded rows usable while artwork is pending", async () => {
    const callbacks: ((result: ArtworkResult) => void)[] = [];
    const release = vi.fn();
    vi.spyOn(artworkQueue, "subscribe").mockImplementation(
      (_id, _priority, callback) => {
        callbacks.push(callback);
        return release;
      },
    );
    render(<LibraryView />);
    await screen.findByText("Game 0");
    fireEvent.keyDown(screen.getByRole("grid"), { key: "End" });
    await screen.findByText("Game 14999");
    expect(screen.getAllByRole("row").length).toBeLessThanOrEqual(28);
    expect(useLibraryUI.getState().selection?.id).toBe("id-14999");
    fireEvent.change(screen.getByRole("searchbox"), {
      target: { value: "Game" },
    });
    await waitFor(() =>
      expect(getLibraryInfo).toHaveBeenLastCalledWith(
        expect.objectContaining({ search: "Game" }),
      ),
    );
    await screen.findByText("Game 0");
    expect(release).toHaveBeenCalled();
    act(() =>
      callbacks[0]({
        status: "corrupt",
        dataUrl: null,
        cache: "none",
        workMs: 1,
        decodeMs: 0,
      }),
    );
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByRole("rowgroup").scrollTop).toBe(0);
    expect(pageGate.activeRequests).toBeLessThanOrEqual(1);
  }, 15000);
  it("resets the virtualizer when a deep query becomes a short result set", async () => {
    vi.mocked(getLibraryInfo).mockImplementation(async (criteria) => ({
      total: criteria?.search ? 5 : total,
      libraryTotal: total,
      sources: [],
      queryMs: 0,
      source: "Synthetic",
      diagnostics: false,
    }));
    vi.mocked(getLibraryWindow).mockImplementation(
      async (offset, _limit, criteria) =>
        criteria?.search
          ? {
              offset,
              items: Array.from({ length: 5 }, (_, i) => row(i)),
              queryMs: 1,
            }
          : response(offset),
    );
    render(<LibraryView />);
    await screen.findByText("Game 0");
    fireEvent.keyDown(screen.getByRole("grid"), { key: "End" });
    await screen.findByText("Game 14999");
    fireEvent.change(screen.getByRole("searchbox"), {
      target: { value: "small" },
    });
    await screen.findByText("Game 0");
    expect(screen.getByRole("rowgroup").scrollTop).toBe(0);
    expect(screen.getAllByRole("row")).toHaveLength(6);
    fireEvent.keyDown(screen.getByRole("grid"), { key: "End" });
    expect(useLibraryUI.getState().selection?.id).toBe("id-4");
  });
  it("keeps source options usable while new counts load without showing stale rows", async () => {
    const metadata = {
      total: 15000,
      libraryTotal: 15000,
      source: "Synthetic",
      diagnostics: false,
      sources: [{ id: "s1", name: "One" }],
      queryMs: 0,
    };
    vi.mocked(getLibraryInfo).mockResolvedValue(metadata);
    render(<LibraryView />);
    await screen.findByText("Game 0");
    let finish!: (value: typeof metadata) => void;
    vi.mocked(getLibraryInfo).mockImplementationOnce(
      () =>
        new Promise((r) => {
          finish = r;
        }),
    );
    fireEvent.change(
      screen.getByRole("combobox", { name: "Installation filter" }),
      { target: { value: "installed" } },
    );
    await waitFor(() => expect(getLibraryInfo).toHaveBeenCalledTimes(2));
    expect(screen.queryByRole("grid")).toBeNull();
    expect(screen.getByRole("option", { name: "One" })).toBeTruthy();
    fireEvent.change(
      screen.getByRole("combobox", { name: "Library source filter" }),
      { target: { value: "s1" } },
    );
    finish(metadata);
    await waitFor(() =>
      expect(getLibraryInfo).toHaveBeenLastCalledWith(
        expect.objectContaining({ installed: "installed", sourceId: "s1" }),
      ),
    );
    await screen.findByText("Game 0");
  });
  it("debounces search, immediately hides old results, and clears selection", async () => {
    render(<LibraryView />);
    await screen.findByText("Game 0");
    fireEvent.click(screen.getByText("Game 0"));
    const search = screen.getByRole("searchbox");
    search.focus();
    vi.useFakeTimers();
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "a" } });
    fireEvent.change(screen.getByRole("searchbox"), {
      target: { value: "alpha" },
    });
    expect(screen.queryByText("Game 0")).toBeNull();
    expect(useLibraryUI.getState().selection).toBeNull();
    await act(async () => {
      await vi.advanceTimersByTimeAsync(149);
    });
    expect(getLibraryInfo).toHaveBeenCalledTimes(1);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10);
    });
    expect(getLibraryInfo).toHaveBeenLastCalledWith(
      expect.objectContaining({ search: "alpha" }),
    );
    expect(getLibraryInfo).toHaveBeenCalledTimes(2);
    expect(document.activeElement).toBe(search);
  });

  it("composes filters and sorting, distinguishes no matches, and resets criteria", async () => {
    vi.mocked(getLibraryInfo).mockImplementation(async (criteria) => ({
      total: criteria?.search ? 0 : 15000,
      libraryTotal: 15000,
      queryMs: 0,
      source: "Synthetic",
      diagnostics: false,
      sources: [{ id: "s1", name: "One" }],
    }));
    render(<LibraryView />);
    await screen.findByText("Game 0");
    fireEvent.change(
      screen.getByRole("combobox", { name: "Installation filter" }),
      { target: { value: "installed" } },
    );
    await waitFor(() =>
      expect(getLibraryInfo).toHaveBeenLastCalledWith(
        expect.objectContaining({ installed: "installed" }),
      ),
    );
    await screen.findByText("Game 0");
    fireEvent.change(
      screen.getByRole("combobox", { name: "Library source filter" }),
      { target: { value: "s1" } },
    );
    fireEvent.change(
      screen.getByRole("spinbutton", { name: "Release year from" }),
      { target: { value: "2000" } },
    );
    fireEvent.change(screen.getByRole("combobox", { name: "Sort games" }), {
      target: { value: "releaseDate:desc" },
    });
    await waitFor(() =>
      expect(getLibraryInfo).toHaveBeenLastCalledWith(
        expect.objectContaining({
          installed: "installed",
          sourceId: "s1",
          yearFrom: 2000,
          sort: "releaseDate",
          direction: "desc",
        }),
      ),
    );
    fireEvent.change(screen.getByRole("searchbox"), {
      target: { value: "absent" },
    });
    await screen.findByText(/No games match these criteria/);
    expect(screen.queryByText(/Your library is empty/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Reset all" }));
    await screen.findByText("Game 0");
    expect(useLibraryUI.getState().criteria).toEqual(defaultCriteria);
    expect(screen.getAllByRole("row").length).toBeLessThanOrEqual(28);
  });

  it("rejects an abandoned query's late page and starts the new criteria at zero", async () => {
    const view = render(<LibraryView />);
    await screen.findByText("Game 0");
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
    fireEvent.change(screen.getByRole("searchbox"), {
      target: { value: "new" },
    });
    expect(screen.queryByRole("grid")).toBeNull();
    finish(response(1000));
    await waitFor(() =>
      expect(getLibraryInfo).toHaveBeenLastCalledWith(
        expect.objectContaining({ search: "new" }),
      ),
    );
    await screen.findByText("Game 0");
    expect(screen.queryByText("Game 1000")).toBeNull();
    expect(screen.getByRole("rowgroup").scrollTop).toBe(0);
    expect(getLibraryWindow).toHaveBeenLastCalledWith(
      0,
      100,
      expect.objectContaining({ search: "new" }),
    );
    view.unmount();
    expect(pageGate.queued).toBe(0);
  });
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
    // 99 synchronous jsdom scroll commits are a correctness stress scenario,
    // not a wall-clock performance measurement. Native timings are benchmarked.
  }, 15000);

  it("ignores out-of-order metadata and cancels queued work on unmount", async () => {
    let oldInfo!: (value: Awaited<ReturnType<typeof getLibraryInfo>>) => void;
    vi.mocked(getLibraryInfo).mockImplementationOnce(
      () =>
        new Promise((r) => {
          oldInfo = r;
        }),
    );
    const view = render(<LibraryView />);
    await waitFor(() => expect(getLibraryInfo).toHaveBeenCalledTimes(1));
    fireEvent.click(screen.getByRole("button", { name: "Refresh library" }));
    oldInfo({
      total: 0,
      libraryTotal: 0,
      sources: [],
      queryMs: 0,
      source: "Stale metadata",
      diagnostics: false,
    });
    await screen.findByText("Game 0");
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
    await waitFor(() => expect(getLibraryInfo).toHaveBeenCalledTimes(1));
    finish({
      total,
      libraryTotal: total,
      sources: [],
      queryMs: 0,
      source: "Synthetic",
      diagnostics: false,
    });
    await screen.findByRole("alert");
    expect(screen.queryByText(/Your library is empty/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Retry library" }));
    await screen.findByText("Game 0");
  });
  it("renders empty and persistent initialization failures distinctly", async () => {
    vi.mocked(getLibraryInfo).mockResolvedValueOnce({
      total: 0,
      libraryTotal: 0,
      sources: [],
      queryMs: 0,
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
      libraryTotal: 500,
      sources: [],
      queryMs: 0,
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
