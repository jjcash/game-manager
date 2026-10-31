// SPDX-License-Identifier: GPL-3.0-or-later
// Ordinary optimized service; file pickers are operated through the native UI.
import assert from "node:assert/strict";
import { readFile, writeFile, unlink, readdir } from "node:fs/promises";
import path from "node:path";
import { connectWebview } from "./webview.mjs";
const [endpoint, output, phase, root] = process.argv.slice(2);
const view = await connectWebview(endpoint);
const invoke = (name, args = {}) =>
  view.evaluate(
    `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(name)},${JSON.stringify(args)})`,
  );
const wait = async (expression) => {
  for (let i = 0; i < 3600; i++) {
    if (await view.evaluate(`Boolean(${expression})`)) return;
    await view.sleep(100);
  }
  throw new Error(`Timed out: ${expression}`);
};
const manifest = path.join(root, "identities.json");
const originals = path.join(root, "application/database/artwork/originals");
const request = async (id) => ({
  gameId: id,
  expectedRevision: (await invoke("manual_get", { gameId: id })).revision,
});
async function manage(id) {
  await view.click("Refresh library");
  await wait(`document.getElementById('game-${id}')`);
  await view.evaluate(`document.getElementById('game-${id}').click()`);
  await view.click("Manage selected game");
  await wait(
    `document.querySelector('dialog[open] fieldset') && !document.querySelector('dialog fieldset').disabled`,
  );
}
async function choose(label, instruction, expression) {
  await writeFile(
    path.join(root, "dialog-step.json"),
    JSON.stringify({ instruction, at: new Date().toISOString() }),
  );
  console.log(instruction);
  await view.click(label);
  await wait(expression);
}
const report = { phase, checks: [] };
try {
  await wait(
    `document.querySelector('.library-panel')?.dataset.queryReady==='true'`,
  );
  assert.equal((await invoke("database_get_status")).schemaVersion, 5);
  if (phase === "restart") {
    const expected = JSON.parse(await readFile(manifest, "utf8"));
    assert.deepEqual(
      await invoke("manual_get", { gameId: expected.id }),
      expected,
    );
    await manage(expected.id);
    await wait(`document.querySelector('[aria-label="Local artwork"] img')`);
    assert.equal(
      (await invoke("artwork_get_thumbnail", { id: expected.id })).status,
      "ready",
    );
    const known = path.join(originals, `${expected.artworkAssetId}.img`);
    await unlink(known);
    await view.click("Close editor");
    await manage(expected.id);
    await wait(
      `document.querySelector('[aria-label="Local artwork"] [data-artwork-status]')?.dataset.artworkStatus!=='loading'`,
    );
    assert.equal(
      await view.evaluate(
        `document.querySelector('[aria-label="Local artwork"] img')===null`,
      ),
      true,
    );
    assert(
      (await invoke("manual_get", { gameId: expected.id })).artworkAssetId,
    );
    report.checks.push(
      "restart retains exact assignment",
      "present original renders",
      "removed test original gives placeholder without deleting assignment",
    );
  } else {
    const id = await invoke("manual_create", {
      request: {
        metadata: {
          title: "Local artwork fixture",
          releaseDate: null,
          genre: null,
          developer: null,
          favorite: false,
        },
        sourceName: "Manual",
        installation: null,
      },
    });
    await manage(id);
    await choose(
      "Choose local artwork…",
      "CANCEL the native artwork picker",
      `!document.querySelector('dialog fieldset').disabled`,
    );
    assert.equal((await invoke("manual_get", { gameId: id })).revision, 0);
    await choose(
      "Choose local artwork…",
      `OPEN ${path.join(root, "first.png")}`,
      `document.querySelector('[aria-label="Local artwork"] img')`,
    );
    const first = await invoke("manual_get", { gameId: id });
    assert.equal(first.revision, 1);
    assert(first.artworkAssetId);
    const firstBytes = await readFile(
      path.join(originals, `${first.artworkAssetId}.img`),
    );
    await unlink(path.join(root, "first.png"));
    await view.click("Close editor");
    await manage(id);
    await wait(`document.querySelector('[aria-label="Local artwork"] img')`);
    await choose(
      "Replace local artwork…",
      `OPEN ${path.join(root, "corrupt.png")}`,
      `document.querySelector('dialog [role="alert"]')`,
    );
    assert.deepEqual(await invoke("manual_get", { gameId: id }), first);
    await choose(
      "Replace local artwork…",
      `OPEN ${path.join(root, "second.png")}`,
      `!document.querySelector('dialog fieldset').disabled && !document.querySelector('dialog [role="alert"]')`,
    );
    const second = await invoke("manual_get", { gameId: id });
    assert.equal(second.revision, 2);
    assert.notEqual(first.artworkAssetId, second.artworkAssetId);
    assert.deepEqual(
      await readFile(path.join(originals, `${first.artworkAssetId}.img`)),
      firstBytes,
    );
    await view.click("Clear artwork");
    await wait(
      `document.querySelector('[aria-label="Local artwork"] button')?.textContent.includes('Choose')`,
    );
    assert.equal(
      (await invoke("manual_get", { gameId: id })).artworkAssetId,
      null,
    );
    assert.equal(
      (await readdir(originals)).filter((n) => n.endsWith(".img")).length,
      2,
    );
    await choose(
      "Choose local artwork…",
      `OPEN ${path.join(root, "second.png")} AGAIN`,
      `document.querySelector('[aria-label="Local artwork"] button')?.textContent.includes('Replace')`,
    );
    const final = await invoke("manual_get", { gameId: id });
    assert.equal(final.revision, 4);
    await writeFile(manifest, JSON.stringify(final, null, 2));
    const shot = await view.command("Page.captureScreenshot", {
      format: "png",
    });
    await writeFile(`${output}.png`, Buffer.from(shot.data, "base64"));
    report.checks.push(
      "native picker cancellation unchanged",
      "PNG owned copy survives external source removal",
      "corrupt replacement retains exact record",
      "replacement preserves old originals",
      "clear removes assignment only",
      "new assignment advances game revision",
    );
  }
  report.stats = await invoke("artwork_get_stats");
  await writeFile(output, JSON.stringify(report, null, 2), { flag: "wx" });
  console.log("Local artwork", phase, "passed");
} finally {
  view.socket.close();
}
