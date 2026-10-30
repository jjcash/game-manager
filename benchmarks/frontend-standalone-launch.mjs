// SPDX-License-Identifier: GPL-3.0-or-later
// Ordinary optimized application; only the runner's compiled harmless fixture.
import assert from "node:assert/strict";
import { readFile, writeFile, access, unlink } from "node:fs/promises";
import path from "node:path";
import { connectWebview } from "./webview.mjs";
const [endpoint, output, directory, phase, manifest] = process.argv.slice(2);
const view = await connectWebview(endpoint);
const invoke = (name, args = {}) =>
  view.evaluate(
    `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(name)},${JSON.stringify(args)})`,
  );
const get = (id) => invoke("manual_get", { gameId: id });
const exists = async (file) =>
  access(path.join(directory, file)).then(
    () => true,
    () => false,
  );
const args = [
  "",
  "two words",
  'quote"slash\\',
  "日本語 🎮",
  "%PATH% & | > $(echo literal)",
];
const ready = () =>
  view.until(
    `document.querySelector('.library-panel')?.dataset.queryReady==='true'`,
  );
async function manage(id) {
  await view.click("Refresh library");
  await ready();
  await view.until(`document.getElementById('game-${id}')`);
  await view.evaluate(`document.getElementById('game-${id}').click()`);
  await view.click("Manage selected game");
  await view.until(
    `document.querySelector('dialog[open] fieldset') && !document.querySelector('dialog fieldset').disabled`,
  );
}
const launchButton = `document.querySelector('[aria-label="Standalone launch"] button')`;
try {
  await ready();
  if (phase === "restart") {
    const before = JSON.parse(await readFile(manifest, "utf8"));
    assert.deepEqual(await get(before.id), before);
    assert.equal(
      await exists("received.bin"),
      false,
      "startup must never launch",
    );
    await manage(before.id);
    assert.equal(
      await view.evaluate(
        `document.querySelector('[aria-label="Standalone launch"] select').value`,
      ),
      before.defaultProfileId,
    );
    assert.equal(
      await exists("received.bin"),
      false,
      "details/default preselection must never launch",
    );
  } else {
    assert.equal((await invoke("database_get_status")).counts.games, 0);
    const id = await invoke("manual_create", {
      request: {
        metadata: {
          title: "Synthetic standalone launch",
          releaseDate: null,
          genre: null,
          developer: null,
          favorite: false,
        },
        sourceName: "Manual",
        installation: {
          rootPathRef: directory,
          platform: "windows",
          version: null,
          detectionState: "unknown",
        },
      },
    });
    let d = await get(id);
    await invoke("manual_mutate", {
      request: {
        gameId: id,
        expectedRevision: d.revision,
        action: {
          kind: "launchProfile",
          action: {
            operation: "create",
            installationId: d.installations[0].id,
            configuration: {
              displayName: "Synthetic fixture",
              launcherKind: "standalone",
              executableRef: {
                kind: "installationRelative",
                value: "fixture.exe",
              },
              arguments: args,
              workingDirectoryRef: null,
            },
          },
        },
      },
    });
    d = await get(id);
    await manage(id);
    assert.equal(await view.evaluate(`${launchButton}.disabled`), true);
    assert.equal(await exists("received.bin"), false);
    await view.evaluate(
      `(()=>{const e=document.querySelector('[aria-label="Standalone launch"] select');Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype,'value').set.call(e,${JSON.stringify(d.profiles[0].id)});e.dispatchEvent(new Event('change',{bubbles:true}));})()`,
    );
    assert.equal(
      await exists("received.bin"),
      false,
      "profile selection must never launch",
    );
    await view.until(`!${launchButton}.disabled`);
    await view.evaluate(`${launchButton}.focus()`);
    assert.equal(
      await view.evaluate(`document.activeElement===${launchButton}`),
      true,
    );
    await view.command("Input.dispatchKeyEvent", {
      type: "keyDown",
      key: "Enter",
      code: "Enter",
      windowsVirtualKeyCode: 13,
      text: "\r",
      unmodifiedText: "\r",
    });
    await view.command("Input.dispatchKeyEvent", {
      type: "keyUp",
      key: "Enter",
      code: "Enter",
      windowsVirtualKeyCode: 13,
    });
    await view.until(
      `document.querySelector('[aria-label="Standalone launch"] [role=status]').textContent==='Launch request sent to Windows.'`,
    );
    for (let i = 0; i < 100 && !(await exists("exited.txt")); i++)
      await view.sleep(50);
    assert.equal(await exists("exited.txt"), true);
    const bytes = await readFile(path.join(directory, "received.bin"));
    const received = [];
    for (let offset = 0; offset < bytes.length;) {
      const n = bytes.readUInt32LE(offset);
      offset += 4;
      received.push(bytes.subarray(offset, offset + n).toString("utf8"));
      offset += n;
    }
    assert.deepEqual(received.slice(5), args);
    assert.equal(received[1], directory);
    assert.equal(received[4], "noInput");
    assert.equal(
      (await get(id)).defaultProfileId,
      null,
      "one-time selection must not save a default",
    );
    const request = {
      gameId: id,
      profileId: d.profiles[0].id,
      expectedProfileRevision: d.profiles[0].revision,
      expectedGameRevision: d.revision,
    };
    assert(
      await view.evaluate(
        `window.__TAURI_INTERNALS__.invoke('standalone_launch',{request:${JSON.stringify({ ...request, executablePath: "untrusted.exe" })}}).then(()=>false,()=>true)`,
      ),
    );
    // Change the authoritative game/default while the open details remain stale.
    await invoke("manual_mutate", {
      request: {
        gameId: id,
        expectedRevision: d.revision,
        action: {
          kind: "launchProfile",
          action: {
            operation: "setDefault",
            profileId: d.profiles[0].id,
            expectedProfileRevision: d.profiles[0].revision,
          },
        },
      },
    });
    await unlink(path.join(directory, "received.bin"));
    await unlink(path.join(directory, "exited.txt"));
    await view.click("Launch");
    await view.until(
      `document.querySelector('[aria-label="Standalone launch"] [role=alert]')?.textContent.includes('Configuration changed')`,
    );
    await view.until(`${launchButton}.disabled`);
    assert.equal(
      await exists("received.bin"),
      false,
      "stale request/refresh must never launch",
    );
    d = await get(id);
    await writeFile(manifest, JSON.stringify(d, null, 2), { flag: "wx" });
  }
  const geometry = await view.evaluate(
    `(()=>{const d=document.querySelector('dialog');return {overflow:d.scrollWidth>d.clientWidth,launchButtons:d.querySelectorAll('[aria-label="Standalone launch"] button').length};})()`,
  );
  assert.equal(geometry.overflow, false);
  assert.equal(geometry.launchButtons, 1);
  await writeFile(
    output,
    JSON.stringify(
      {
        phase,
        checks: [
          "explicit selection/default",
          "no automatic invocation",
          "keyboard launch",
          "literal production IPC",
          "creation-only response",
          "stale refresh",
          "restart persistence",
        ],
        geometry,
      },
      null,
      2,
    ),
    { flag: "wx" },
  );
  const shot = await view.command("Page.captureScreenshot", { format: "png" });
  await writeFile(`${output}.png`, Buffer.from(shot.data, "base64"), {
    flag: "wx",
  });
  await view.key("Escape", 27);
} catch (error) {
  await writeFile(
    `${output}.failure.json`,
    JSON.stringify(
      await view.evaluate(
        `({text:document.querySelector('[aria-label="Standalone launch"]')?.textContent,focus:document.activeElement?.outerHTML})`,
      ),
      null,
      2,
    ),
  );
  const shot = await view.command("Page.captureScreenshot", { format: "png" });
  await writeFile(`${output}.failure.png`, Buffer.from(shot.data, "base64"));
  throw error;
} finally {
  view.socket.close();
}
