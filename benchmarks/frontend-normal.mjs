// SPDX-License-Identifier: GPL-3.0-or-later
// Normal-build smoke through the existing isolated, DB-hash-checking runner.
import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { connectWebview } from "./webview.mjs";
const [endpoint, output] = process.argv.slice(2);
const { evaluate, until, command, socket } = await connectWebview(endpoint);
try {
  await until(
    `document.body.textContent.includes('Your library is empty') && document.body.textContent.includes('Rust backend connected')`,
  );
  const result = await evaluate(
    `(async()=>{const invoke=window.__TAURI_INTERNALS__.invoke;return {status:await invoke('app_get_status'),info:await invoke('library_get_info'),database:await invoke('database_get_status'),artwork:await invoke('artwork_get_thumbnail',{id:'syn-000000000000002a-g-00001'}),diagnostics:document.querySelectorAll('.diagnostics').length,alerts:document.querySelectorAll('[role="alert"]').length}})()`,
  );
  assert.equal(result.status.backend, "rust");
  assert.equal(result.info.total, 0);
  assert.equal(result.info.diagnostics, false);
  assert.equal(result.info.source, "Local application library");
  assert.equal(result.artwork.status, "missing");
  assert.equal(result.diagnostics, 0);
  assert.equal(result.alerts, 0);
  await writeFile(output, JSON.stringify(result, null, 2), { flag: "wx" });
  const shot = await command("Page.captureScreenshot", { format: "png" });
  await writeFile(`${output}.png`, Buffer.from(shot.data, "base64"));
  console.log(
    "Normal optimized launch, empty isolated library, Rust IPC and disabled benchmark selection: passed.",
  );
} finally {
  socket.close();
}
