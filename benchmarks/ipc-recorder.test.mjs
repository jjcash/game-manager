// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import test from "node:test";
import { startIpcRecorder } from "./ipc-recorder.mjs";
function connection() {
  let listener;
  const commands = [];
  return {
    commands,
    onEvent(fn) {
      listener = fn;
      return () => {
        listener = null;
      };
    },
    emit(method, params) {
      listener?.({ method, params });
    },
    async command(name, args) {
      commands.push(name);
      if (name === "Network.getResponseBody")
        return {
          body: JSON.stringify({
            queryMs: 2,
            poolWaitMs: 0.1,
            items: [{ id: args.requestId }],
          }),
        };
      return {};
    },
  };
}
test("records bounded protocol intervals and narrow metadata without retaining headers/bodies", async () => {
  const c = connection(),
    recorder = await startIpcRecorder({ ...c, timeOrigin: 100000 }, 2);
  for (let i = 0; i < 3; i++) {
    c.emit("Network.requestWillBeSent", {
      requestId: String(i),
      timestamp: i,
      wallTime: 100 + i,
      request: {
        method: "POST",
        url: "http://ipc.localhost/library_query",
        headers: { secret: "omit" },
        postData: JSON.stringify({ request: { offset: i, limit: 100 } }),
      },
    });
    c.emit("Network.loadingFinished", {
      requestId: String(i),
      timestamp: i + 0.01,
    });
    await new Promise((resolve) => setImmediate(resolve));
  }
  const events = await recorder.stop();
  assert.deepEqual(
    events.map((e) => e.offset),
    [1, 2],
  );
  assert(
    events.every(
      (e) => e.rows === 1 && Math.abs(e.endedAt - e.startedAt - 10) < 0.0001,
    ),
  );
  assert(
    events.every(
      (e) => !Object.hasOwn(e, "headers") && !Object.hasOwn(e, "body"),
    ),
  );
  assert.deepEqual(c.commands.slice(-1), ["Network.disable"]);
});
test("ignores unrelated traffic and retains failed or incomplete requests", async () => {
  const c = connection(),
    recorder = await startIpcRecorder({ ...c, timeOrigin: 0 });
  const request = (id, url) =>
    c.emit("Network.requestWillBeSent", {
      requestId: id,
      timestamp: 1,
      wallTime: 1,
      request: { method: "POST", url },
    });
  request("other", "http://example.invalid/library_query");
  request("failed", "http://ipc.localhost/library_get_info");
  c.emit("Network.loadingFailed", { requestId: "failed", timestamp: 2 });
  request("pending", "http://ipc.localhost/library_query");
  const events = await recorder.stop();
  assert.equal(events.length, 2);
  assert.equal(events[0].failed, true);
  assert.equal(events[1].incomplete, true);
});
