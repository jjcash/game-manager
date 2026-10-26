// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it, vi } from "vitest";
import { RequestGate } from "./requestGate";
import { createLibraryClient, prunePages } from "./cache";
function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}
describe("bounded library work", () => {
  it("drops superseded queued work, holds a cancelled native permit, and rejects late results", async () => {
    const gate = new RequestGate();
    const a = new AbortController();
    const b = new AbortController();
    const c = new AbortController();
    const native = deferred<number>();
    const work = vi.fn(() => native.promise);
    const first = gate.run(a.signal, work);
    const failure = expect(first).rejects.toBeTruthy();
    await Promise.resolve();
    const skipped = vi.fn(async () => 2);
    const second = gate.run(b.signal, skipped);
    const secondFailure = expect(second).rejects.toBeTruthy();
    b.abort();
    a.abort();
    expect(gate.queued).toBe(0);
    const latest = vi.fn(async () => 3);
    const third = gate.run(c.signal, latest);
    expect(latest).not.toHaveBeenCalled();
    native.resolve(1);
    await failure;
    await secondFailure;
    expect(await third).toBe(3);
    expect(skipped).not.toHaveBeenCalled();
  });
  it("evicts inactive pages by count and reuses retained data", async () => {
    const client = createLibraryClient();
    for (let i = 0; i < 20; i++) {
      client.setQueryData(["page", 0, i], { items: [i] });
      prunePages(client);
    }
    expect(client.getQueryCache().findAll({ queryKey: ["page"] })).toHaveLength(
      6,
    );
    const load = vi.fn();
    expect(
      await client.fetchQuery({ queryKey: ["page", 0, 19], queryFn: load }),
    ).toEqual({ items: [19] });
    expect(load).not.toHaveBeenCalled();
    client.clear();
  });
});
