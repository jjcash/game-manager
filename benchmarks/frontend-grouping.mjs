// SPDX-License-Identifier: GPL-3.0-or-later
// New disposable app root only. All location values are opaque synthetic metadata.
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
const get = (id) => invoke("manual_get", { gameId: id });
const mutate = async (id, action) => {
  const d = await get(id);
  await invoke("manual_mutate", {
    request: { gameId: id, expectedRevision: d.revision, action },
  });
};
const ready = () =>
  until(
    `document.querySelector('.library-panel')?.dataset.queryReady==='true'`,
  );
const editorReady = () =>
  until(
    `document.querySelector('dialog[open] fieldset') && !document.querySelector('dialog fieldset').disabled`,
  );
const field = (label, value) =>
  evaluate(
    `(()=>{const l=Array.from(document.querySelectorAll('dialog label')).find(l=>Array.from(l.childNodes).filter(n=>n.nodeType===Node.TEXT_NODE).map(n=>n.textContent).join('').trim()===${JSON.stringify(label)});const e=l?.querySelector('input,select');if(!e)throw Error('Missing field '+${JSON.stringify(label)});Object.getOwnPropertyDescriptor(e.tagName==='SELECT'?HTMLSelectElement.prototype:HTMLInputElement.prototype,'value').set.call(e,${JSON.stringify(value)});e.dispatchEvent(new Event('input',{bubbles:true}));e.dispatchEvent(new Event('change',{bubbles:true}));})()`,
  );
const manage = async (id) => {
  await click("Refresh library");
  await until(
    `document.getElementById(${JSON.stringify(`game-${id}`)}) && document.querySelector('.library-panel')?.dataset.queryReady==='true'`,
  );
  await evaluate(
    `document.getElementById(${JSON.stringify(`game-${id}`)}).click()`,
  );
  await click("Manage selected game");
  await editorReady();
  await evaluate(`document.querySelector('.installation-groups').open=true`);
};
const save = async (label, id) => {
  const d = await get(id);
  await click(label);
  await until(
    `window.__TAURI_INTERNALS__.invoke('manual_get',{gameId:${JSON.stringify(id)}}).then(d=>d.revision>${d.revision})`,
  );
  await editorReady();
};
const shot = async (suffix) => {
  const r = await command("Page.captureScreenshot", { format: "png" });
  await writeFile(`${output}.${suffix}.png`, Buffer.from(r.data, "base64"), {
    flag: "wx",
  });
};
try {
  await ready();
  assert.equal((await invoke("database_get_status")).schemaVersion, 4);
  if (phase === "organize") {
    const metadata = {
      title: "日本語 Café — copies",
      releaseDate: null,
      genre: null,
      developer: null,
      favorite: false,
    };
    const ids = [];
    for (let n = 0; n < 2; n++)
      ids.push(
        await invoke("manual_create", {
          request: { metadata, sourceName: "Manual", installation: null },
        }),
      );
    const game = ids[0];
    await mutate(game, { kind: "addEntry", sourceName: "Second membership" });
    let d = await get(game);
    for (const [index, location] of [
      [0, "metadata://copy-a"],
      [0, "metadata://copy-b"],
      [1, "metadata://copy-c"],
    ])
      await mutate(game, {
        kind: "saveInstallation",
        entryId: d.entries[index].id,
        installationId: null,
        installation: {
          rootPathRef: location,
          platform: "windows",
          version: null,
          detectionState: "unknown",
        },
      });
    await manage(game);
    await field("Group name", "Cancelled draft");
    await click("Cancel group edit");
    assert.equal((await get(game)).groups.length, 0);
    for (let n = 0; n < 2; n++) {
      await field("Group name", "日本語 copies 🎮");
      await save("Create group", game);
    }
    d = await get(game);
    assert.equal(d.groups.length, 2);
    assert(
      d.groups.every(
        (g) =>
          g.displayName.includes(String.fromCodePoint(0x1f3ae)) &&
          g.displayName.includes(String.fromCodePoint(0x65e5)),
      ),
    );
    assert.notEqual(d.groups[0].id, d.groups[1].id);
    const group = d.groups[0].id;
    await field("Group to edit", group);
    await field("Group name", "Preferred copies 🎮");
    await save("Save group name", game);
    for (const i of d.installations) {
      await field("Installation to organize", i.id);
      await field("Assigned group", group);
      await save("Save group assignment", game);
    }
    d = await get(game);
    assert(
      d.installations.every(
        (i) => i.groupId === group && i.detectionState === "unknown",
      ),
    );
    await field("Installation to organize", d.installations[0].id);
    await field("Assigned group", "");
    await save("Save group assignment", game);
    await field("Group to edit", group);
    await field("Group name", "Stale draft kept");
    await mutate(game, { kind: "metadata", metadata: d.metadata });
    await click("Save group name");
    await until(
      `document.querySelector('dialog [role=alert]')?.textContent.includes('Game changed')`,
    );
    assert.equal(
      await evaluate(
        `document.querySelector('.installation-groups input').value`,
      ),
      "Stale draft kept",
    );
    assert.equal(
      (await get(game)).groups.find((g) => g.id === group).displayName,
      "Preferred copies 🎮",
    );
    await shot("conflict");
    await click("Close editor");
    await manage(game);
    for (let n = 0; n < 18; n++) {
      await key("Tab", 9);
      assert(
        await evaluate(
          `document.querySelector('dialog').contains(document.activeElement)`,
        ),
      );
    }
    const geometry = await evaluate(
      `(()=>{const d=document.querySelector('dialog'),r=d.getBoundingClientRect();return {visible:r.x>=0&&r.y>=0&&r.right<=innerWidth&&r.bottom<=innerHeight,overflow:d.scrollWidth>d.clientWidth};})()`,
    );
    assert(geometry.visible);
    assert.equal(geometry.overflow, false);
    await field("Group to edit", group);
    await field("Installation to organize", d.installations[1].id);
    await evaluate(
      `document.querySelector('.installation-groups').scrollIntoView({block:'center'})`,
    );
    await shot("groups");
    const expected = { ids, details: await Promise.all(ids.map(get)), group };
    await writeFile(manifest, JSON.stringify(expected, null, 2), {
      flag: "wx",
    });
    await click("Close editor");
    await writeFile(
      output,
      JSON.stringify(
        {
          phase,
          geometry,
          checks:
            "duplicate titles/IDs, duplicate Unicode groups, rename, assignments across memberships, ungroup, cancellation, stale revision, focus and viewport",
        },
        null,
        2,
      ),
      { flag: "wx" },
    );
  } else {
    const expected = JSON.parse(await readFile(manifest, "utf8"));
    assert.deepEqual(
      await Promise.all(expected.ids.map(get)),
      expected.details,
    );
    const game = expected.ids[0];
    await manage(game);
    await field("Group to edit", expected.group);
    await click("Remove group");
    assert(
      await evaluate(
        `document.querySelector('[role=alertdialog]').textContent.includes('become ungrouped')`,
      ),
    );
    assert.equal(
      await evaluate(`document.activeElement.textContent.trim()`),
      "Keep records",
    );
    await key("Tab", 9);
    assert.equal(
      await evaluate(`document.activeElement.textContent.trim()`),
      "Confirm removal",
    );
    await shot("remove-confirmation");
    await click("Keep records");
    assert.equal((await get(game)).groups.length, 2);
    await field("Group to edit", expected.group);
    await click("Remove group");
    await save("Confirm removal", game);
    const d = await get(game);
    assert.equal(d.groups.length, 1);
    assert.equal(d.installations.length, 3);
    assert(
      d.installations.every(
        (i) => i.groupId === null && i.detectionState === "unknown",
      ),
    );
    await click("Close editor");
    await ready();
    assert.equal((await invoke("database_get_status")).counts.games, 2);
    await writeFile(
      output,
      JSON.stringify(
        {
          phase,
          checks:
            "exact restart identities, explicit removal cancellation/confirmation, preserved installations/presence/counts",
          details: d,
        },
        null,
        2,
      ),
      { flag: "wx" },
    );
  }
} finally {
  socket.close();
}
