// SPDX-License-Identifier: GPL-3.0-or-later
// Passive bounded CDP protocol timing. Tauri's immutable invoke is untouched.
export async function startIpcRecorder(
  { command, onEvent, timeOrigin },
  capacity = 2048,
) {
  if (!Number.isInteger(capacity) || capacity < 1 || capacity > 2048)
    throw new RangeError("Invalid recorder capacity");
  const events = [],
    requests = new Map(),
    pending = new Set();
  const remove = onEvent((message) => {
    const p = message.params;
    if (message.method === "Network.requestWillBeSent") {
      let url;
      try {
        url = new URL(p.request.url);
      } catch {
        return;
      }
      const name = url.pathname.slice(1);
      if (
        url.hostname !== "ipc.localhost" ||
        p.request.method !== "POST" ||
        !["library_query", "library_get_info"].includes(name)
      )
        return;
      let args = {};
      try {
        args = JSON.parse(p.request.postData ?? "{}");
      } catch {
        /* preserve timing without payload */
      }
      const event = {
        command: name,
        startedAt: p.wallTime * 1000 - timeOrigin,
        offset: args.request?.offset ?? null,
        limit: args.request?.limit ?? null,
        criteria: args.request?.criteria ?? args.criteria ?? null,
      };
      if (events.length === capacity) events.shift();
      events.push(event);
      if (requests.size === capacity) {
        const first = requests.keys().next().value;
        requests.get(first).event.incomplete = true;
        requests.delete(first);
      }
      requests.set(p.requestId, { event, timestamp: p.timestamp });
    } else if (
      ["Network.loadingFinished", "Network.loadingFailed"].includes(
        message.method,
      )
    ) {
      const request = requests.get(p.requestId);
      if (!request) return;
      requests.delete(p.requestId);
      const event = request.event;
      event.endedAt =
        event.startedAt + (p.timestamp - request.timestamp) * 1000;
      if (message.method === "Network.loadingFailed") {
        event.failed = true;
        return;
      }
      if (pending.size === capacity) {
        event.bodyUnavailable = "Recorder readback capacity";
        return;
      }
      const read = command("Network.getResponseBody", {
        requestId: p.requestId,
      })
        .then((body) => {
          const reply = JSON.parse(
            body.base64Encoded
              ? Buffer.from(body.body, "base64").toString("utf8")
              : body.body,
          );
          event.serviceMs = reply.queryMs;
          event.poolMs = reply.poolWaitMs;
          event.rows = reply.items?.length;
        })
        .catch((error) => {
          event.bodyUnavailable = String(error?.message ?? error);
        });
      pending.add(read);
      read.finally(() => pending.delete(read));
    }
  });
  await command("Network.enable", {
    maxTotalBufferSize: 4 * 1024 * 1024,
    maxResourceBufferSize: 512 * 1024,
    maxPostDataSize: 16384,
  });
  return {
    async stop() {
      await Promise.all([...pending]);
      remove();
      await command("Network.disable");
      for (const { event } of requests.values()) event.incomplete = true;
      return events;
    },
  };
}
