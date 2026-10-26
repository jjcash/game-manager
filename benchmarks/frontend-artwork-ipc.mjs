// SPDX-License-Identifier: GPL-3.0-or-later
// Isolated native IPC contention probe, separate from normal UI scenario timings.
import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import { connectWebview } from "./webview.mjs";
const [endpoint = "http://127.0.0.1:9223", output] = process.argv.slice(2);
const { evaluate, until, socket } = await connectWebview(endpoint);
let report;
try {
  await until(
    `document.querySelector('.library-panel[data-query-ready="true"]')`,
  );
  await until(
    `!document.querySelector('.thumbnail[data-artwork-status="loading"]')`,
  );
  report = await evaluate(`(async()=>{
    const invoke=window.__TAURI_INTERNALS__.invoke;
    const base={search:'',installed:'all',sourceId:null,yearFrom:null,yearTo:null,sort:'title',direction:'asc'};
    const total=Number(document.querySelector('.library-panel').dataset.resultCount);
    const report={total,iterations:50,startedAt:performance.now(),timeOrigin:performance.timeOrigin,cases:[],artworkRequests:0,artworkMisses:0,artworkFailures:0};
    // Title-sorted windows can share patterned source subsets. Ordinal probes
    // deliberately exceed both cache capacities before timed contention work.
    for(let i=0;i<Math.min(total,800);i+=2){await Promise.all([i,i+1].map(index=>invoke('artwork_get_thumbnail',{id:'syn-000000000000002a-g-'+String(index).padStart(5,'0')})));}
    report.afterSaturation=await invoke('artwork_get_stats');
    let stop=false;
    const artwork=(async()=>{let index=0;while(!stop){const ids=[index%total,(index+1)%total].map(i=>'syn-000000000000002a-g-'+String(i).padStart(5,'0'));const results=await Promise.all(ids.map(id=>invoke('artwork_get_thumbnail',{id})));for(const r of results){report.artworkRequests++;if(r.cache==='miss')report.artworkMisses++;if(r.status!=='ready')report.artworkFailures++;}index+=2;}})();
    const cases=[['default',base],['installed',{...base,installed:'installed'}],['notInstalled',{...base,installed:'notInstalled'}],['combined',{...base,search:'chron',installed:'installed',sourceId:'archive',yearFrom:1990,yearTo:2020,sort:'releaseDate',direction:'desc'}]];
    try{
      for(const [name,criteria] of cases){
        const first=await invoke('library_get_info',{criteria});
        const request={limit:100,offset:Math.max(0,first.total-100),criteria};
        const firstWindow=await invoke('library_query',{request});
        const ids=JSON.stringify(firstWindow.items.map(r=>r.id));
        const metadata=[],windows=[],countQuery=[],pageQuery=[];
        for(let i=0;i<50;i++){
          let start=performance.now();const info=await invoke('library_get_info',{criteria});metadata.push(performance.now()-start);countQuery.push(info.queryMs);
          start=performance.now();const page=await invoke('library_query',{request});windows.push(performance.now()-start);pageQuery.push(page.queryMs);
          if(info.total!==first.total||page.items.length>100||JSON.stringify(page.items.map(r=>r.id))!==ids)throw new Error('Unstable IPC result');
        }
        report.cases.push({name,count:first.total,metadata,windows,countQuery,pageQuery});
      }
    }finally{stop=true;await artwork;}
    report.native=await invoke('artwork_get_stats');report.durationMs=performance.now()-report.startedAt;
    return report;
  })()`);
  assert.equal(report.cases.length, 4);
  assert.equal(report.afterSaturation.memoryEntries, 128);
  assert.equal(report.afterSaturation.diskEntries, 256);
  assert(report.artworkRequests > 0 && report.artworkMisses > 0);
  assert(report.native.maxActive <= 2 && report.native.maxActiveDecodes <= 2);
  assert(
    report.native.diskEntries <= 256 && report.native.memoryEntries <= 128,
  );
} finally {
  socket.close();
  if (output && report)
    await writeFile(output, JSON.stringify(report, null, 2), { flag: "wx" });
  console.log(
    JSON.stringify({
      total: report?.total,
      cases: report?.cases?.length,
      durationMs: report?.durationMs,
    }),
  );
}
