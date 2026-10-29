// Measure real browser import and scroll frames for one-million-character JSON.
// Usage: node bench/ui-browser.mjs [before|after] [pretty|minified]
import {writeFile} from 'node:fs/promises';
const label=process.argv[2]??'after', pretty=process.argv[3]!=='minified';
const row=pretty?'  {\n    "id": 12345,\n    "name": "JSON Lens performance example",\n    "active": true,\n    "data": [1, 2, 3]\n  }':'{"id":12345,"name":"JSON Lens performance example","active":true,"data":[1,2,3]}';
const source='[\n'+Array(Math.floor(1_000_000/(row.length+2))).fill(row).join(pretty?',\n':',')+'\n]';
const file=`/private/tmp/json-lens-ui-${pretty?'pretty':'minified'}.json`;
await writeFile(file,source);
const tabs=await (await fetch('http://127.0.0.1:9227/json/list')).json();
const ws=new WebSocket(tabs.find(tab=>tab.type==='page').webSocketDebuggerUrl);
await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});
let id=0;const pending=new Map(),logs=[],errors=[];
ws.onmessage=event=>{const msg=JSON.parse(event.data);if(msg.id){const p=pending.get(msg.id);pending.delete(msg.id);if(msg.error)p.reject(msg.error);else p.resolve(msg.result);}else if(msg.method==='Runtime.consoleAPICalled')logs.push(msg.params.args.map(arg=>arg.value??arg.description??'').join(' '));else if(msg.method==='Runtime.exceptionThrown')errors.push(msg.params.exceptionDetails);};
const call=(method,params={})=>new Promise((resolve,reject)=>{const request=++id;pending.set(request,{resolve,reject});ws.send(JSON.stringify({id:request,method,params}));});
const evaluate=async expression=>{const r=await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw r.exceptionDetails;return r.result.value;};
const pause=ms=>evaluate(`new Promise(r=>setTimeout(()=>r(true),${ms}))`);
const click=async(x,y)=>{await call('Input.dispatchMouseEvent',{type:'mousePressed',x,y,button:'left',clickCount:1});await call('Input.dispatchMouseEvent',{type:'mouseReleased',x,y,button:'left',clickCount:1});await pause(100);};
const startProbe=()=>evaluate(`(()=>{if(window.uiProbe)window.uiProbe.running=false;const p=window.uiProbe={running:true,gaps:[],last:performance.now(),start:performance.now()};const pulse=now=>{if(!p.running)return;p.gaps.push(now-p.last);p.last=now;requestAnimationFrame(pulse)};requestAnimationFrame(pulse)})()`);
const stopProbe=()=>evaluate(`(()=>{const p=window.uiProbe;p.running=false;const gaps=p.gaps.filter(n=>n>=0).sort((a,b)=>a-b);return {frames:gaps.length,median_gap_ms:gaps[Math.floor(gaps.length/2)],p95_gap_ms:gaps[Math.floor(gaps.length*.95)],max_gap_ms:Math.max(...gaps),over_50_ms:gaps.filter(n=>n>50).length}})()`);
try{
  await call('Runtime.enable');await call('Page.enable');
  await call('Emulation.setDeviceMetricsOverride',{width:1280,height:800,deviceScaleFactor:1,mobile:false});
  await call('Page.navigate',{url:'http://127.0.0.1:8081/json-lens/?debug=1'});
  if(!await evaluate(`(async()=>{for(let i=0;i<120;i++){if(window.wasmBindings&&!document.getElementById('loading'))return true;await new Promise(r=>setTimeout(r,250));}return false})()`))throw new Error('App failed to load');
  await pause(500);
  logs.length=0;
  await click(1040,25);
  const dom=await call('DOM.getDocument');const input=await call('DOM.querySelector',{nodeId:dom.root.nodeId,selector:'#rfd-input'});
  if(!input.nodeId)throw new Error('File picker missing');
  await call('DOM.setFileInputFiles',{nodeId:input.nodeId,files:[file]});
  await startProbe();
  await evaluate(`[...document.querySelectorAll('#rfd-card button')].find(button=>button.textContent==='Ok').click()`);
  await pause(2500);
  const opening=await stopProbe();
  await startProbe();
  for(let i=0;i<16;i++){await call('Input.dispatchMouseEvent',{type:'mouseWheel',x:300,y:400,deltaX:0,deltaY:160});await pause(50);}
  await pause(100);const scrolling=await stopProbe();
  if(errors.length)throw new Error(JSON.stringify(errors));
  const environment=await evaluate('({user_agent:navigator.userAgent,device_pixel_ratio:devicePixelRatio,cross_origin_isolated:crossOriginIsolated})');
  const result={label,pretty,bytes:source.length,environment,opening,scrolling};
  await writeFile(`/private/tmp/json-lens-browser-ui-${label}-${pretty?'pretty':'minified'}.json`,JSON.stringify(result,null,2)+'\n');
  await writeFile(`/private/tmp/json-lens-browser-ui-${label}-${pretty?'pretty':'minified'}.log`,logs.filter(line=>line.includes('[JSON Lens')).join('\n')+'\n');
  console.log(JSON.stringify(result,null,2));
}finally{ws.close();}
