// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
export async function connectWebview(endpoint) {
  assert.equal(new URL(endpoint).hostname, "127.0.0.1");
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  let targets;
  const matches = (p) =>
    p.type === "page" &&
    /^(http:\/\/tauri\.localhost\/|http:\/\/127\.0\.0\.1:1420\/)/.test(p.url);
  for (let i = 0; i < 100; i++) {
    try {
      targets = await (await fetch(`${endpoint}/json/list`)).json();
      if (targets.some(matches)) break;
    } catch {
      /* starting */
    }
    await sleep(100);
  }
  const target = targets?.find(matches);
  assert(target, "Expected a Game Manager WebView page");
  const socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((r, reject) => {
    socket.onopen = r;
    socket.onerror = reject;
  });
  let sequence = 0;
  const pending = new Map();
  const listeners = new Set();
  socket.onmessage = (event) => {
    const m = JSON.parse(event.data);
    if (m.id) {
      const p = pending.get(m.id);
      pending.delete(m.id);
      if (m.error) p.reject(new Error(JSON.stringify(m.error)));
      else p.resolve(m.result);
    } else for (const listener of listeners) listener(m);
  };
  function command(method, params = {}) {
    return new Promise((resolve, reject) => {
      const id = ++sequence;
      pending.set(id, { resolve, reject });
      socket.send(JSON.stringify({ id, method, params }));
    });
  }
  async function evaluate(expression) {
    const r = await command("Runtime.evaluate", {
      expression,
      returnByValue: true,
      awaitPromise: true,
    });
    if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails));
    return r.result.value;
  }
  async function until(expression) {
    for (let i = 0; i < 200; i++) {
      const r = await evaluate(`(async()=>Boolean(await (${expression})))()`);
      if (r) return r;
      await sleep(50);
    }
    throw new Error(`Timed out: ${expression}`);
  }
  const click = (label) =>
    evaluate(
      `Array.from(document.querySelectorAll('button')).find(b=>b.textContent.trim()===${JSON.stringify(label)})?.click()`,
    );
  const loaded = `document.querySelector('.game-row:not([aria-busy="true"])') && !document.querySelector('.library-scroll[aria-busy="true"]')`;
  const snapshot = () =>
    evaluate(
      `JSON.parse(document.querySelector('[aria-label="Performance measurements"]').textContent)`,
    );
  async function key(name, code) {
    await command("Input.dispatchKeyEvent", {
      type: "keyDown",
      key: name,
      code: name,
      windowsVirtualKeyCode: code,
    });
    await command("Input.dispatchKeyEvent", {
      type: "keyUp",
      key: name,
      code: name,
      windowsVirtualKeyCode: code,
    });
  }

  return {
    onEvent(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    evaluate,
    until,
    click,
    key,
    command,
    snapshot,
    loaded,
    socket,
    target,
    sleep,
  };
}
