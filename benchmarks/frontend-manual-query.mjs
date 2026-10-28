// SPDX-License-Identifier: GPL-3.0-or-later
// Bounded query regression on a disposable application-purpose copy of synthetic data.
import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { connectWebview } from "./webview.mjs";
const [endpoint, output, expectedSchema] = process.argv.slice(2);
const { evaluate, until, socket } = await connectWebview(endpoint);
try {
  await until(
    `document.querySelector('.library-panel')?.dataset.queryReady==='true'`,
  );
  const result = await evaluate(`(async()=>{
    const invoke=window.__TAURI_INTERNALS__.invoke;
    const status=await invoke('database_get_status');const sources=(await invoke('library_get_info')).sources;
    const results=[];
    for(const [name,criteria] of [['all',{}],['installed',{installed:'installed'}],['notInstalled',{installed:'notInstalled'}],['source',{sourceId:sources[0].id}],['search',{search:'star'}],['release',{sort:'releaseDate',direction:'desc'}]]) {
      const info=await invoke('library_get_info',{criteria});
      for(const offset of [0,Math.max(0,info.total-100)]) {
        const samples=[];let first;
        for(let i=0;i<21;i++) {
          const start=performance.now();const page=await invoke('library_query',{request:{offset,limit:100,criteria}});
          if(i===0) first=page.items;else samples.push({queryMs:page.queryMs,invokeMs:performance.now()-start});
          if(JSON.stringify(first)!==JSON.stringify(page.items))throw new Error('Unstable page');
        }
        results.push({name,offset,total:info.total,ids:first.map(r=>r.id),rows:first,samples});
      }
    }
    return {status,results};
  })()`);
  assert.equal(result.status.purpose, "application");
  assert.equal(result.status.schemaVersion, Number(expectedSchema));
  assert.equal(result.status.foreignKeys, true);
  for (const r of result.results)
    assert.equal(r.ids.length, Math.min(100, r.total - r.offset));
  await writeFile(output, JSON.stringify(result, null, 2), { flag: "wx" });
  console.log(
    `Application schema ${expectedSchema}, ${result.status.counts.games} games: passed`,
  );
} finally {
  socket.close();
}
