// SPDX-License-Identifier: GPL-3.0-or-later
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { ArtworkQueue, artworkQueue, type ArtworkResult } from "./artwork";
import { Thumbnail } from "./Thumbnail";
const ready: ArtworkResult = {
  status: "ready",
  dataUrl: "data:image/png;base64,AAAA",
  cache: "miss",
  workMs: 1,
  decodeMs: 1,
};
const deferred = () => {
  let resolve!: (r: ArtworkResult) => void;
  const promise = new Promise<ArtworkResult>((r) => {
    resolve = r;
  });
  return { promise, resolve };
};
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
});
it("prioritizes visible jobs, deduplicates, cancels offscreen work and bounds admission", async () => {
  vi.useFakeTimers();
  const pending = deferred();
  const load = vi.fn((id: string) => {
    expect(id).toBeTruthy();
    return pending.promise;
  });
  const queue = new ArtworkQueue(load);
  const removed = vi.fn();
  const first = vi.fn();
  const duplicate = vi.fn();
  queue.subscribe("near", 1, vi.fn());
  queue.subscribe("visible", 0, first);
  queue.subscribe("visible", 0, duplicate);
  const release = queue.subscribe("removed", 0, removed);
  release();
  await vi.advanceTimersByTimeAsync(16);
  expect(load.mock.calls.map((c) => c[0])).toEqual(["visible", "near"]);
  for (let i = 0; i < 100; i++) queue.subscribe(`distant-${i}`, 1, vi.fn());
  expect(queue.active).toBe(2);
  expect(queue.queued).toBe(32);
  expect(queue.dropped).toBeGreaterThan(0);
  pending.resolve(ready);
  await vi.advanceTimersByTimeAsync(1000);
  expect(first).toHaveBeenCalledOnce();
  expect(duplicate).toHaveBeenCalledOnce();
  expect(removed).not.toHaveBeenCalled();
  expect(queue.maxActive).toBe(2);
  expect(queue.maxQueued).toBe(32);
});
it("bounds cache occupancy, reuses hits, expires sources and recovers after failure", async () => {
  vi.useFakeTimers();
  const load = vi.fn(async () => ready);
  const queue = new ArtworkQueue(load);
  for (let i = 0; i < 140; i++) {
    queue.subscribe(`id-${i}`, 0, vi.fn());
    await vi.advanceTimersByTimeAsync(17);
  }
  expect(queue.cached).toBe(128);
  const count = load.mock.calls.length;
  queue.subscribe("id-139", 0, vi.fn());
  expect(load).toHaveBeenCalledTimes(count);
  queue.subscribe("id-0", 0, vi.fn());
  await vi.advanceTimersByTimeAsync(17);
  expect(load).toHaveBeenCalledTimes(count + 1);
  // Cache freshness uses the same performance clock as native rendering.
  vi.spyOn(performance, "now").mockReturnValue(1000000);
  queue.subscribe("id-139", 0, vi.fn());
  await vi.advanceTimersByTimeAsync(17);
  expect(load).toHaveBeenCalledTimes(count + 2);
  load.mockRejectedValueOnce(new Error("unavailable"));
  const failed = vi.fn();
  queue.subscribe("failure", 0, failed);
  await vi.advanceTimersByTimeAsync(17);
  expect(failed.mock.calls[0][0].status).toBe("failed");
  queue.clear();
  const recovered = vi.fn();
  queue.subscribe("failure", 0, recovered);
  await vi.advanceTimersByTimeAsync(17);
  expect(recovered).toHaveBeenCalledWith(ready);
});
it("released running work never updates stale rows and a changed identity hides old results", async () => {
  const callbacks = new Map<string, (result: ArtworkResult) => void>();
  const released = vi.fn();
  vi.spyOn(artworkQueue, "subscribe").mockImplementation(
    (id, _priority, listener) => {
      callbacks.set(id, listener);
      return released;
    },
  );
  const { rerender, container } = render(<Thumbnail id="old" visible />);
  expect(container.querySelector("img")).toBeNull();
  rerender(<Thumbnail id="new" visible />);
  expect(released).toHaveBeenCalledOnce();
  act(() => callbacks.get("old")!(ready));
  expect(container.querySelector("img")).toBeNull();
  act(() => callbacks.get("new")!(ready));
  expect(container.querySelector("img")?.getAttribute("width")).toBe("48");
  fireEvent.error(container.querySelector("img")!);
  expect(container.querySelector("img")).toBeNull();
  rerender(<Thumbnail id="new" visible revision={1} />);
  act(() => callbacks.get("new")!(ready));
  expect(container.querySelector("img")).not.toBeNull();
  expect(screen.queryByRole("alert")).toBeNull();
});
it("does not notify released active subscribers", async () => {
  vi.useFakeTimers();
  const pending = deferred();
  const queue = new ArtworkQueue(() => pending.promise);
  const listener = vi.fn();
  const release = queue.subscribe("old", 0, listener);
  await vi.advanceTimersByTimeAsync(16);
  release();
  pending.resolve(ready);
  await vi.advanceTimersByTimeAsync(16);
  expect(listener).not.toHaveBeenCalled();
  expect(queue.active).toBe(0);
});
it("supports zero or one artwork worker without an unbounded queue", async () => {
  vi.useFakeTimers();
  const pending = deferred();
  const load = vi.fn(() => pending.promise);
  const queue = new ArtworkQueue(load);
  queue.configure(0);
  const listener = vi.fn();
  queue.subscribe("disabled", 0, listener);
  expect(listener).toHaveBeenCalledWith(
    expect.objectContaining({ status: "missing" }),
  );
  expect(load).not.toHaveBeenCalled();
  queue.configure(1);
  queue.subscribe("first", 0, vi.fn());
  queue.subscribe("second", 0, vi.fn());
  await vi.advanceTimersByTimeAsync(16);
  expect(queue.active).toBe(1);
  expect(queue.queued).toBe(1);
  pending.resolve(ready);
  await vi.advanceTimersByTimeAsync(50);
  expect(queue.maxActive).toBe(1);
});
