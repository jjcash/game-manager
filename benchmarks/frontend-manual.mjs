// SPDX-License-Identifier: GPL-3.0-or-later
// Ordinary production executable, disposable application DB, real UI writes.
import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import { connectWebview } from "./webview.mjs";
const [endpoint, output, phase, manifest] = process.argv.slice(2);
const { evaluate, until, click, command, key, socket } =
  await connectWebview(endpoint);
const invoke = (name, args = {}) =>
  evaluate(
    `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(name)},${JSON.stringify(args)})`,
  );
const field = (label, value, scope = "dialog") =>
  evaluate(`(()=>{
  const root=document.querySelector(${JSON.stringify(scope)});const label=Array.from(root.querySelectorAll('label')).find(l=>Array.from(l.childNodes).filter(n=>n.nodeType===Node.TEXT_NODE).map(n=>n.textContent).join('').trim()===${JSON.stringify(label)});const e=label?.querySelector('input,select');if(!e)throw new Error('Missing field');Object.getOwnPropertyDescriptor(e.tagName==='SELECT'?HTMLSelectElement.prototype:HTMLInputElement.prototype,'value').set.call(e,${JSON.stringify(value)});e.dispatchEvent(new Event('input',{bubbles:true}));e.dispatchEvent(new Event('change',{bubbles:true}));})()`);
const ready = () =>
  until(
    `document.querySelector('.library-panel')?.dataset.queryReady==='true'`,
  );
const editorReady = () =>
  until(
    `document.querySelector('dialog[open] fieldset') && !document.querySelector('dialog fieldset').disabled`,
  );
const count = (n) =>
  until(
    `document.querySelector('.library-panel')?.dataset.queryReady==='true' && document.querySelector('.library-panel').dataset.resultCount==='${n}'`,
  );
const manage = async (id) => {
  await ready();
  await evaluate(
    `document.getElementById(${JSON.stringify(`game-${id}`)}).click()`,
  );
  await click("Manage selected game");
  await editorReady();
};
const save = async (label, id, revision) => {
  await click(label);
  await until(
    `window.__TAURI_INTERNALS__.invoke('manual_get',{gameId:${JSON.stringify(id)}}).then(d=>d.revision>${revision})`,
  );
  await editorReady();
};
try {
  await ready();
  const initial = await invoke("database_get_status");
  assert.equal(initial.purpose, "application");
  assert.equal(initial.schemaVersion, 2);
  assert.equal((await invoke("library_get_info")).diagnostics, false);
  assert.equal(
    await evaluate(
      `window.__TAURI_INTERNALS__.invoke('synthetic_start',{}).then(()=>false,()=>true)`,
    ),
    true,
  );
  if (phase === "create") {
    await count(0);
    for (let i = 0; i < 2; i++) {
      await click("Add game");
      await editorReady();
      if (i === 0) {
        const geometry = await evaluate(
          `(()=>{const d=document.querySelector('dialog'),r=d.getBoundingClientRect();return {x:r.x,y:r.y,right:r.right,bottom:r.bottom,width:innerWidth,height:innerHeight,horizontalOverflow:d.scrollWidth>d.clientWidth};})()`,
        );
        assert(
          geometry.x > 0 &&
            geometry.y > 0 &&
            geometry.right <= geometry.width &&
            geometry.bottom <= geometry.height,
        );
        assert.equal(geometry.horizontalOverflow, false);
        for (let tab = 0; tab < 14; tab++) {
          await key("Tab", 9);
          assert.equal(
            await evaluate(
              `document.querySelector('dialog').contains(document.activeElement)`,
            ),
            true,
          );
        }
        await field("Title", "   ");
        await click("Create game");
        await until(`document.querySelector('dialog [role="alert"]')`);
        assert.equal((await invoke("database_get_status")).counts.games, 0);
      }
      await field("Title", "日本語 Café %_ ' Manual");
      if (i === 0) {
        await field("Release date (YYYY-MM-DD)", "2024-02-29");
        await field("Genre", "Adventure");
        await evaluate(
          `Array.from(document.querySelectorAll('dialog label')).find(l=>l.textContent.includes('Record an installation')).querySelector('input').click()`,
        );
        await field(
          "Installation location (metadata only)",
          "Z:\\M1.1-unaccessed\\game",
        );
        await field("Recorded presence", "present");
      }
      await click("Create game");
      await until(`!document.querySelector('dialog')`);
      await count(i + 1);
    }
    const page = await invoke("library_query", {
      request: { limit: 100, offset: 0, criteria: {} },
    });
    assert.equal(page.items.length, 2);
    assert.notEqual(page.items[0].id, page.items[1].id);
    const details = await Promise.all(
      page.items.map((r) => invoke("manual_get", { gameId: r.id })),
    );
    const game = details.find((d) => d.installations.length === 1),
      other = details.find((d) => d.installations.length === 0);
    assert(game && other);
    await writeFile(
      manifest,
      JSON.stringify({ game: game.id, other: other.id }, null, 2),
      { flag: "wx" },
    );
  } else {
    const { game, other } = JSON.parse(await readFile(manifest, "utf8"));
    if (phase === "edit") {
      await count(2);
      let d = await invoke("manual_get", { gameId: game });
      assert.equal(d.metadata.title, "日本語 Café %_ ' Manual");
      assert.equal(d.metadata.releaseDate, "2024-02-29");
      assert.equal(d.installations[0].stateOrigin, "user");
      assert.equal(d.installations[0].detectionState, "present");
      await field("Search titles", "Manual", ".library-controls");
      await count(2);
      await manage(game);
      await field("Completion for Manual", "playing");
      await save("Save membership", game, d.revision);
      d = await invoke("manual_get", { gameId: game });
      assert.equal(d.entries[0].completionStatus, "playing");
      await field("Title", "ZZ User edited 🎮");
      await field("Developer", "User studio");
      await save("Save metadata", game, d.revision);
      d = await invoke("manual_get", { gameId: game });
      await count(1);
      await field("Manual source name", "Collection B");
      await save("Add membership", game, d.revision);
      d = await invoke("manual_get", { gameId: game });
      assert.equal(d.entries.length, 2);
      assert.equal(d.metadata.title, "ZZ User edited 🎮");
      await field(
        "Installation membership",
        d.entries.find((e) => e.sourceName === "Collection B").id,
      );
      await field(
        "Installation location (metadata only)",
        "Z:\\M1.1-unaccessed\\second",
      );
      await field("Version", "1.2");
      await field("Recorded presence", "unknown");
      await save("Add installation record", game, d.revision);
      d = await invoke("manual_get", { gameId: game });
      assert.equal(d.installations.length, 2);
      assert.equal(d.overriddenFields.length, 5);
      const screenshot = await command("Page.captureScreenshot", {
        format: "png",
      });
      await writeFile(`${output}.png`, Buffer.from(screenshot.data, "base64"));
      await evaluate(
        `document.querySelector('dialog').scrollTop=document.querySelector('dialog').scrollHeight`,
      );
      const bottom = await command("Page.captureScreenshot", { format: "png" });
      await writeFile(
        `${output}.bottom.png`,
        Buffer.from(bottom.data, "base64"),
      );
      await click("Close editor");
      await click("Reset all");
      await count(2);
      // Existing source filtering must retain global presence from the other membership.
      await field(
        "Library source",
        d.entries.find((e) => e.sourceName === "Collection B").sourceId,
        ".library-controls",
      );
      await field("Installation", "installed", ".library-controls");
      await count(1);
      assert.equal(
        await evaluate(`document.querySelector('.game-title').textContent`),
        "ZZ User edited 🎮",
      );
    } else if (phase === "remove") {
      await count(2);
      const d = await invoke("manual_get", { gameId: game });
      assert.equal(d.metadata.title, "ZZ User edited 🎮");
      assert.equal(d.metadata.developer, "User studio");
      assert.equal(d.entries.length, 2);
      assert.equal(d.installations.length, 2);
      await manage(game);
      await click("Remove installation record");
      assert(
        await evaluate(
          `document.querySelector('[role="alertdialog"]').textContent.includes('Files remain untouched')`,
        ),
      );
      await save("Confirm removal", game, d.revision);
      let next = await invoke("manual_get", { gameId: game });
      assert.equal(next.installations.length, 1);
      await click("Remove membership");
      await save("Confirm removal", game, next.revision);
      next = await invoke("manual_get", { gameId: game });
      assert.equal(next.entries.length, 1);
      await click("Remove manual game");
      await click("Confirm removal");
      await until(`!document.querySelector('dialog')`);
      await count(1);
      await manage(other);
      await click("Remove manual game");
      await click("Confirm removal");
      await until(`!document.querySelector('dialog')`);
      await count(0);
      const missing = await evaluate(
        `window.__TAURI_INTERNALS__.invoke('manual_get',{gameId:${JSON.stringify(game)}}).then(()=>null,e=>e.code)`,
      );
      assert.equal(missing, "NOT_FOUND");
    } else if (phase === "empty") {
      await count(0);
      assert.equal((await invoke("database_get_status")).counts.games, 0);
    } else throw new Error("Unknown phase");
  }
  assert.equal(
    await evaluate(`document.querySelectorAll('[role="alert"]').length`),
    0,
  );
  await writeFile(
    output,
    JSON.stringify(
      { phase, status: await invoke("database_get_status"), passed: true },
      null,
      2,
    ),
    { flag: "wx" },
  );
  console.log(`Manual native ${phase}: passed`);
} finally {
  socket.close();
}
