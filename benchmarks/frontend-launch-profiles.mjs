// SPDX-License-Identifier: GPL-3.0-or-later
// Disposable native app metadata only. Every reference is synthetic and unopened.
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
    `(()=>{const l=Array.from(document.querySelectorAll('dialog label')).find(l=>Array.from(l.childNodes).filter(n=>n.nodeType===Node.TEXT_NODE).map(n=>n.textContent).join('').trim()===${JSON.stringify(label)});const e=l?.querySelector('input,select,textarea');if(!e)throw Error('Missing '+${JSON.stringify(label)});const p=e.tagName==='SELECT'?HTMLSelectElement.prototype:e.tagName==='TEXTAREA'?HTMLTextAreaElement.prototype:HTMLInputElement.prototype;Object.getOwnPropertyDescriptor(p,'value').set.call(e,${JSON.stringify(value)});e.dispatchEvent(new Event('input',{bubbles:true}));e.dispatchEvent(new Event('change',{bubbles:true}));})()`,
  );
const waitData = (id, expression) =>
  until(
    `window.__TAURI_INTERNALS__.invoke('manual_get',{gameId:${JSON.stringify(id)}}).then(d=>${expression})`,
  );
async function manage(id) {
  await click("Refresh library");
  await ready();
  await until(`document.getElementById('game-${id}')`);
  await evaluate(`document.getElementById('game-${id}').click()`);
  await click("Manage selected game");
  await editorReady();
  await evaluate(
    `document.querySelector('.launch-profiles').open=true;document.querySelector('.launch-profiles').scrollIntoView()`,
  );
}
async function shot(suffix) {
  const r = await command("Page.captureScreenshot", { format: "png" });
  await writeFile(`${output}.${suffix}.png`, Buffer.from(r.data, "base64"), {
    flag: "wx",
  });
}
const checks = [];
try {
  await ready();
  assert.equal((await invoke("database_get_status")).schemaVersion, 4);
  if (phase === "configure") {
    assert.equal((await invoke("database_get_status")).counts.games, 0);
    const id = await invoke("manual_create", {
      request: {
        metadata: {
          title: "Launch configuration 日本語 🎮",
          releaseDate: null,
          genre: null,
          developer: null,
          favorite: false,
        },
        sourceName: "Manual",
        installation: {
          rootPathRef: "Z:\\synthetic-unopened",
          platform: "windows",
          version: null,
          detectionState: "unknown",
        },
      },
    });
    let d = await get(id);
    const installation = d.installations[0].id;
    await manage(id);
    await field("Profile installation", installation);
    await field("Profile name", "日本語 duplicate 🎮");
    await field("Executable reference", "bin\\ゲーム.exe");
    await field("Literal arguments (JSON array)", "[1]");
    await click("Create profile");
    await until(`document.querySelector('.launch-profiles [role=alert]')`);
    assert.equal((await get(id)).profiles.length, 0);
    const args = ["two words", '"quoted"', "", "日本語 🎮", "%TEMP% & |"];
    await field("Literal arguments (JSON array)", JSON.stringify(args));
    await click("Create profile");
    await waitData(id, "d.profiles.length===1");
    await editorReady();
    d = await get(id);
    const first = d.profiles[0].id;
    assert.deepEqual(d.profiles[0].configuration.arguments, args);
    assert.equal(d.revision, 0);
    assert.equal(d.profiles[0].revision, 0);
    const invalidRequest = {
      request: {
        gameId: id,
        expectedRevision: d.revision,
        action: {
          kind: "launchProfile",
          action: {
            operation: "save",
            profileId: first,
            expectedProfileRevision: 0,
            configuration: {
              ...d.profiles[0].configuration,
              executableRef: {
                kind: "installationRelative",
                value: "..\\escape.exe",
              },
            },
          },
        },
      },
    };
    assert.equal(
      await evaluate(
        `window.__TAURI_INTERNALS__.invoke('manual_mutate',${JSON.stringify(invalidRequest)}).then(()=>null,e=>e.code)`,
      ),
      "INVALID_REQUEST",
    );
    assert.equal((await get(id)).profiles[0].revision, 0);
    await field("Profile installation", installation);
    await field("Profile name", "日本語 duplicate 🎮");
    await field("Executable reference", "bin\\ゲーム.exe");
    await field("Literal arguments (JSON array)", JSON.stringify(args));
    await click("Create profile");
    await waitData(id, "d.profiles.length===2");
    await editorReady();
    const existing = (await get(id)).profiles.map((p) => p.id);
    await click(`Duplicate profile ${first.slice(0, 8)}`);
    await waitData(id, "d.profiles.length===3");
    await editorReady();
    d = await get(id);
    const duplicate = d.profiles.find((p) => !existing.includes(p.id));
    assert.deepEqual(
      duplicate.configuration,
      d.profiles.find((p) => p.id === first).configuration,
    );
    await click(`Edit profile ${first.slice(0, 8)}`);
    assert(
      await evaluate(
        `document.querySelector('.launch-profiles select:disabled')!==null`,
      ),
    );
    await field("Executable reference kind", "absolutePath");
    await field("Executable reference", "X:\\synthetic\\ゲーム.exe");
    await evaluate(
      `document.querySelector('.launch-profiles input[type=checkbox]').click()`,
    );
    await field("Working-directory reference kind", "absolutePath");
    await field("Working-directory reference", "X:\\synthetic");
    await evaluate(
      `document.querySelector('.launch-profiles form').scrollIntoView({block:'start'})`,
    );
    await shot("configuration");
    await click("Save profile configuration");
    await waitData(id, `d.profiles.find(p=>p.id==='${first}').revision===1`);
    await editorReady();
    await field("Default LaunchProfile", first);
    await waitData(id, `d.defaultProfileId==='${first}'`);
    await editorReady();
    await click("Clear default profile");
    await waitData(id, "d.defaultProfileId===null");
    await editorReady();
    await field("Default LaunchProfile", duplicate.id);
    await waitData(id, `d.defaultProfileId==='${duplicate.id}'`);
    await editorReady();
    await click(`Remove profile ${duplicate.id.slice(0, 8)}`);
    await until(`document.querySelector('[role=alertdialog]')`);
    await click("Keep records");
    assert.equal((await get(id)).profiles.length, 3);
    await click(`Remove profile ${duplicate.id.slice(0, 8)}`);
    await until(`document.querySelector('[role=alertdialog]')`);
    await click("Confirm removal");
    await waitData(id, "d.profiles.length===2 && d.defaultProfileId===null");
    await editorReady();
    await field("Default LaunchProfile", first);
    await waitData(id, `d.defaultProfileId==='${first}'`);
    await editorReady();
    const geometry = await evaluate(
      `(()=>{const d=document.querySelector('dialog'),r=d.getBoundingClientRect();return {visible:r.x>=0&&r.y>=0&&r.right<=innerWidth&&r.bottom<=innerHeight,overflow:d.scrollWidth>d.clientWidth};})()`,
    );
    assert(geometry.visible);
    assert.equal(geometry.overflow, false);
    await evaluate(
      `document.querySelector('.launch-profiles').scrollIntoView({block:'start'})`,
    );
    await shot("profiles");
    await click("Remove installation record");
    await until(`document.querySelector('[role=alertdialog]')`);
    await click("Confirm removal");
    await until(
      `document.querySelector('dialog [role=alert]')?.textContent.includes('LaunchProfiles')`,
    );
    assert.equal((await get(id)).installations.length, 1);
    await click("Keep records");
    await evaluate(
      `document.querySelector('.launch-profiles textarea').focus()`,
    );
    for (let n = 0; n < 80; n++) {
      await key("Tab", 9);
      assert(
        await evaluate(
          `document.querySelector('dialog').contains(document.activeElement)`,
        ),
      );
    }
    d = await get(id);
    await writeFile(manifest, JSON.stringify(d, null, 2), { flag: "wx" });
    checks.push(
      "native create/invalid input/edit/duplicate/literal arrays/default select/clear/remove confirmation/deletion refusal/keyboard",
    );
    await key("Escape", 27);
    await until(`!document.querySelector('dialog[open]')`);
  } else {
    const before = JSON.parse(await readFile(manifest, "utf8"));
    assert.deepEqual(await get(before.id), before);
    await manage(before.id);
    await shot("restart");
    checks.push(
      "restart preserves all IDs, literal configuration, profile and game revisions, default",
    );
    await key("Escape", 27);
  }
  await writeFile(output, JSON.stringify({ phase, checks }, null, 2), {
    flag: "wx",
  });
} finally {
  socket.close();
}
