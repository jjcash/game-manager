// SPDX-License-Identifier: GPL-3.0-or-later
// No Emulation commands: requires an actual Windows display-scale change.
import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { connectWebview } from "./webview.mjs";

const [output, expectedDpr] = process.argv.slice(2);
const { evaluate, until, command, socket } = await connectWebview(
  "http://127.0.0.1:9223",
);
const report = { timestamp: new Date().toISOString(), emulated: false };
try {
  await until(
    `document.querySelector('.library-panel[data-query-ready="true"]')`,
  );
  report.viewport = await evaluate(
    `({width:innerWidth,height:innerHeight,dpr:devicePixelRatio})`,
  );
  assert.equal(report.viewport.dpr, Number(expectedDpr));
  await evaluate(`(()=>{
    const e=document.querySelector('[aria-label="Sort games"]');
    Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype,'value').set.call(e,'releaseDate:desc');
    e.dispatchEvent(new Event('change',{bubbles:true}));
  })()`);
  await until(`(()=>{
    const p=document.querySelector('.library-panel');
    return p?.dataset.queryReady==='true' && JSON.parse(p.dataset.queryKey).sort==='releaseDate'
      && Array.from(document.querySelectorAll('.game-title')).some(e=>e.textContent.includes(String.fromCodePoint(0x661f)));
  })()`);
  await evaluate(`(()=>{
    const title=Array.from(document.querySelectorAll('.game-title')).find(e=>e.textContent.includes(String.fromCodePoint(0x661f)));
    title.dataset.gmFontAudit='title';
    const placeholder=document.querySelector('.thumbnail span');
    if(!placeholder)throw Error('Expected no-artwork placeholder');
    placeholder.dataset.gmFontAudit='placeholder';
    title.getBoundingClientRect();placeholder.getBoundingClientRect();
  })()`);
  await command("DOM.enable");
  await command("CSS.enable");
  const { root } = await command("DOM.getDocument");
  report.fonts = {};
  for (const label of ["title", "placeholder"]) {
    const { nodeId } = await command("DOM.querySelector", {
      nodeId: root.nodeId,
      selector: `[data-gm-font-audit="${label}"]`,
    });
    report.fonts[label] = await command("CSS.getPlatformFontsForNode", {
      nodeId,
    });
    assert(report.fonts[label].fonts.length);
  }
  report.geometry = await evaluate(`(()=>{
    const rows=Array.from(document.querySelectorAll('.game-row[aria-busy="false"]'));
    return {rows:rows.length,heights:rows.map(e=>e.getBoundingClientRect().height),
      horizontalOverflow:document.documentElement.scrollWidth>innerWidth,
      titleFont:getComputedStyle(document.querySelector('.game-title')).fontFamily,
      placeholderFont:getComputedStyle(document.querySelector('.thumbnail span')).fontFamily};
  })()`);
  assert(report.geometry.rows > 0 && report.geometry.rows <= 40);
  assert(report.geometry.heights.every((h) => Math.abs(h - 38) < 0.1));
  assert(!report.geometry.horizontalOverflow);
  const screenshot = await command("Page.captureScreenshot", { format: "png" });
  await writeFile(`${output}.png`, Buffer.from(screenshot.data, "base64"), {
    flag: "wx",
  });
} catch (error) {
  report.failure = error.message;
  throw error;
} finally {
  socket.close();
  await writeFile(output, JSON.stringify(report, null, 2), { flag: "wx" });
}
