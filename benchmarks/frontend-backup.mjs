// SPDX-License-Identifier: GPL-3.0-or-later
// Real ordinary-binary IPC/UI. Native file pickers are operated interactively;
// the driver never adds a production path-taking command or bypasses validation.
import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import { connectWebview } from "./webview.mjs";
const [endpoint, output, phase, manifest] = process.argv.slice(2);
let view = await connectWebview(endpoint);
const invoke = (name, args = {}) =>
  view.evaluate(
    `window.__TAURI_INTERNALS__.invoke(${JSON.stringify(name)},${JSON.stringify(args)})`,
  );
const wait = async (expression) => {
  for (let i = 0; i < 6000; i++) {
    if (await view.evaluate(`Boolean(${expression})`)) return;
    await view.sleep(100);
  }
  throw new Error(`Timed out: ${expression}`);
};
const screenshot = async (suffix) => {
  const result = await view.command("Page.captureScreenshot", {
    format: "png",
  });
  await writeFile(
    `${output}.${suffix}.png`,
    Buffer.from(result.data, "base64"),
    { flag: "wx" },
  );
};
const click = async (label) => {
  await wait(
    `Array.from(document.querySelectorAll('button')).some(b=>b.textContent.trim()===${JSON.stringify(label)} && !b.disabled)`,
  );
  await view.click(label);
};
const text = (text) =>
  `document.querySelector('.backup-panel')?.textContent.includes(${JSON.stringify(text)})`;
const metadata = {
  title: "ゲーム Café — duplicate",
  releaseDate: null,
  genre: null,
  developer: null,
  favorite: false,
};
const details = async (ids) =>
  Promise.all(ids.map((id) => invoke("manual_get", { gameId: id })));
const report = { phase, dialogs: [], checks: [] };
try {
  await wait(
    `document.querySelector('.library-panel')?.dataset.queryReady==='true'`,
  );
  if (phase === "verify") {
    const expected = JSON.parse(await readFile(manifest, "utf8"));
    assert.deepEqual(await details(expected.ids), expected.details);
    assert.equal((await invoke("database_get_status")).counts.games, 2);
    await click("Settings");
    await wait(text("Restore completed and verified"));
    await screenshot("restart-verified");
    report.checks.push(
      "second real process launch preserves identities and overrides",
    );
  } else {
    assert.equal((await invoke("database_get_status")).counts.games, 0);
    const ids = [];
    for (let i = 0; i < 2; i++)
      ids.push(
        await invoke("manual_create", {
          request: {
            metadata,
            sourceName: "Manual 日本語",
            installation:
              i === 0
                ? {
                    rootPathRef: "metadata://fixture-only",
                    platform: "windows",
                    version: null,
                    detectionState: "present",
                  }
                : null,
          },
        }),
      );
    let d = await invoke("manual_get", { gameId: ids[0] });
    await invoke("manual_mutate", {
      request: {
        gameId: d.id,
        expectedRevision: d.revision,
        action: { kind: "addEntry", sourceName: "Second source" },
      },
    });
    d = await invoke("manual_get", { gameId: ids[0] });
    await invoke("manual_mutate", {
      request: {
        gameId: d.id,
        expectedRevision: d.revision,
        action: {
          kind: "updateEntry",
          entryId: d.entries[0].id,
          completionStatus: "completed",
        },
      },
    });
    // V3 grouping must round-trip with every original M1.4 assertion intact.
    d = await invoke("manual_get", { gameId: ids[0] });
    await invoke("manual_mutate", {
      request: {
        gameId: d.id,
        expectedRevision: d.revision,
        action: {
          kind: "saveGroup",
          groupId: null,
          displayName: "Backup copies 🎮",
        },
      },
    });
    d = await invoke("manual_get", { gameId: ids[0] });
    await invoke("manual_mutate", {
      request: {
        gameId: d.id,
        expectedRevision: d.revision,
        action: {
          kind: "assignGroup",
          installationId: d.installations[0].id,
          groupId: d.groups[0].id,
        },
      },
    });
    // V4 configuration/defaults must restore byte-for-byte logical values too.
    d = await invoke("manual_get", { gameId: ids[0] });
    await invoke("manual_mutate", {
      request: {
        gameId: d.id,
        expectedRevision: d.revision,
        action: {
          kind: "launchProfile",
          action: {
            operation: "create",
            installationId: d.installations[0].id,
            configuration: {
              displayName: "Backup profile",
              launcherKind: "standalone",
              executableRef: {
                kind: "installationRelative",
                value: "bin\\synthetic.exe",
              },
              arguments: ["two words", "", '"quoted"'],
              workingDirectoryRef: {
                kind: "absolutePath",
                value: "Z:\\synthetic-unopened",
              },
            },
          },
        },
      },
    });
    d = await invoke("manual_get", { gameId: ids[0] });
    await invoke("manual_mutate", {
      request: {
        gameId: d.id,
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
    const expected = { ids, details: await details(ids) };
    await writeFile(manifest, JSON.stringify(expected, null, 2), {
      flag: "wx",
    });
    await click("Settings");
    await wait(`!document.querySelector('.backup-actions button').disabled`);
    console.log(
      "PICKER 1: Save a NEW backup at the smoke root / library-backup.sqlite3",
    );
    await click("Create backup…");
    await wait(text("Backup created and verified"));
    report.dialogs.push("native save + verified publication");
    await screenshot("backup");
    console.log("PICKER 2: Cancel the open dialog with Escape");
    await click("Select backup to restore…");
    await wait(text("Restore selection cancelled"));
    report.dialogs.push("native selection cancellation");
    console.log(
      "PICKER 3: Select smoke-root / benchmark-purpose.sqlite3; expect purpose refusal",
    );
    await click("Select backup to restore…");
    await wait(`document.querySelector('.backup-panel [role=alert]')`);
    assert(
      await view.evaluate(
        text("Synthetic benchmark databases cannot be restored"),
      ),
    );
    assert.equal((await invoke("backup_get_status")).candidate, null);
    report.dialogs.push("benchmark-purpose rejection");
    console.log(
      "PICKER 4: Save at the SAME existing backup path; expect collision refusal",
    );
    await click("Create backup…");
    await wait(`document.querySelector('.backup-panel [role=alert]')`);
    assert(await view.evaluate(text("overwriting is disabled")));
    report.dialogs.push("destination collision preserved existing backup");
    d = await invoke("manual_get", { gameId: ids[0] });
    await invoke("manual_mutate", {
      request: {
        gameId: d.id,
        expectedRevision: d.revision,
        action: {
          kind: "metadata",
          metadata: { ...metadata, title: "Current library edit after backup" },
        },
      },
    });
    // Make organization differ too, so restoration must recover its backed-up state.
    d = await invoke("manual_get", { gameId: ids[0] });
    await invoke("manual_mutate", {
      request: {
        gameId: d.id,
        expectedRevision: d.revision,
        action: {
          kind: "saveGroup",
          groupId: d.groups[0].id,
          displayName: "Changed after backup",
        },
      },
    });
    d = await invoke("manual_get", { gameId: ids[0] });
    await invoke("manual_mutate", {
      request: {
        gameId: d.id,
        expectedRevision: d.revision,
        action: {
          kind: "assignGroup",
          installationId: d.installations[0].id,
          groupId: null,
        },
      },
    });
    d = await invoke("manual_get", { gameId: ids[0] });
    await invoke("manual_mutate", {
      request: {
        gameId: d.id,
        expectedRevision: d.revision,
        action: {
          kind: "launchProfile",
          action: {
            operation: "save",
            profileId: d.profiles[0].id,
            expectedProfileRevision: d.profiles[0].revision,
            configuration: {
              ...d.profiles[0].configuration,
              arguments: ["changed after backup"],
              displayName: "Changed profile",
            },
          },
        },
      },
    });
    const extra = await invoke("manual_create", {
      request: {
        metadata: { ...metadata, title: "Added after backup" },
        sourceName: "Manual",
        installation: null,
      },
    });
    const changed = await details(ids);
    console.log(
      "PICKER 5: Select library-backup.sqlite3; review then cancel replacement",
    );
    await click("Select backup to restore…");
    await wait(text("Validated backup"));
    assert.equal(
      (await invoke("backup_get_status")).candidate.snapshot.counts.games,
      2,
    );
    await click("Review replacement…");
    await wait(`!!document.querySelector('dialog[open]')`);
    const bounds = await view.evaluate(
      `(()=>{let d=document.querySelector('dialog'),r=d.getBoundingClientRect();return {visible:r.x>=0&&r.y>=0&&r.right<=innerWidth&&r.bottom<=innerHeight,overflow:d.scrollWidth>d.clientWidth};})()`,
    );
    assert(bounds.visible);
    assert.equal(bounds.overflow, false);
    for (let i = 0; i < 10; i++) {
      await view.key("Tab", 9);
      assert(
        await view.evaluate(
          `document.querySelector('dialog').contains(document.activeElement)`,
        ),
      );
    }
    await screenshot("confirmation");
    await click("Cancel restore");
    await wait(text("Restore cancelled"));
    assert.deepEqual(await details(ids), changed);
    assert.equal((await invoke("database_get_status")).counts.games, 3);
    report.checks.push(
      "confirmation consequences, viewport, Tab focus, cancellation leaves current state intact",
    );
    console.log(
      "PICKER 6: Select library-backup.sqlite3; confirmed replacement follows",
    );
    await click("Select backup to restore…");
    await wait(text("Validated backup"));
    const stale = (await invoke("backup_get_status")).candidate.token - 1;
    const rejected = await view.evaluate(
      `window.__TAURI_INTERNALS__.invoke('backup_confirm_restore',{token:${stale}}).then(()=>false,e=>e.code==='CONFLICT')`,
    );
    assert(rejected);
    const concurrent = await view.evaluate(
      `window.__TAURI_INTERNALS__.invoke('backup_create').then(()=>false,e=>e.code==='CONFLICT')`,
    );
    assert(concurrent);
    await click("Review replacement…");
    await click("Confirm library replacement");
    await wait(text("The library is closed"));
    const status = await invoke("backup_get_status");
    assert(status.restartRequired);
    assert.equal(status.outcome, "pendingRestart");
    assert(
      await view.evaluate(
        `window.__TAURI_INTERNALS__.invoke('manual_create',{request:{metadata:${JSON.stringify(metadata)},sourceName:'Blocked',installation:null}}).then(()=>false,e=>e.code==='NOT_READY')`,
      ),
    );
    await screenshot("prepared");
    report.checks.push(
      "stale confirmation and concurrent backup refused; writes blocked after confirmation",
    );
    const oldId = view.target.id;
    await click("Restart and finish");
    view.socket.close();
    for (let i = 0; i < 300; i++) {
      try {
        const targets = await (await fetch(`${endpoint}/json/list`)).json();
        if (
          targets.some(
            (t) =>
              t.type === "page" &&
              t.id !== oldId &&
              /tauri.localhost/.test(t.url),
          )
        )
          break;
      } catch {
        /* restarting */
      }
      await new Promise((r) => setTimeout(r, 100));
    }
    view = await connectWebview(endpoint);
    await wait(
      `document.querySelector('.library-panel')?.dataset.queryReady==='true'`,
    );
    assert.deepEqual(await details(ids), expected.details);
    assert.equal((await invoke("database_get_status")).counts.games, 2);
    assert(
      await view.evaluate(
        `window.__TAURI_INTERNALS__.invoke('manual_get',{gameId:${JSON.stringify(extra)}}).then(()=>false,e=>e.code==='NOT_FOUND')`,
      ),
    );
    await click("Settings");
    await wait(text("Restore completed and verified"));
    await screenshot("restored");
    report.checks.push(
      "actual Tauri restart, exact identity/override/relationship preservation and replacement verified",
    );
  }
  await writeFile(output, JSON.stringify(report, null, 2), { flag: "wx" });
  console.log("Native backup/restore phase passed:", phase);
} finally {
  view.socket.close();
}
