// SPDX-License-Identifier: GPL-3.0-or-later
import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { connectWebview } from "./webview.mjs";
const [endpoint, output, mode] = process.argv.slice(2);
const { evaluate, until, click, socket } = await connectWebview(endpoint);
try {
  await until(`document.body.textContent.includes('Rust backend connected')`);
  if (mode === "empty") {
    await until(`document.body.textContent.includes('Your library is empty')`);
    const result = await evaluate(
      `window.__TAURI_INTERNALS__.invoke('database_get_status')`,
    );
    assert.equal(result.counts.games, 0);
    assert.equal(result.foreignKeys, true);
    assert.equal(result.journalMode, "wal");
    await writeFile(output, JSON.stringify(result, null, 2), { flag: "wx" });
  } else {
    await until(
      `document.querySelector('[role="alert"]')?.textContent.includes('Database unavailable')`,
    );
    const before = await evaluate(
      `document.querySelector('[role="alert"]').textContent`,
    );
    assert(before.includes("restart Game Manager"));
    await click("Retry library");
    await until(
      `document.querySelector('[role="alert"]')?.textContent.includes('Database unavailable')`,
    );
    const result = await evaluate(
      `(async()=>({status:await window.__TAURI_INTERNALS__.invoke('app_get_status'),rows:document.querySelectorAll('.game-row:not([aria-busy="true"])').length,alert:document.querySelector('[role="alert"]').textContent,code:await window.__TAURI_INTERNALS__.invoke('library_get_info').then(()=>null,e=>e.code)}))()`,
    );
    assert.equal(result.rows, 0);
    assert.equal(result.status.backend, "rust");
    assert(result.code && result.code !== "NOT_READY");
    await writeFile(output, JSON.stringify(result, null, 2), { flag: "wx" });
  }
} finally {
  socket.close();
}
