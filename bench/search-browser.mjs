// Actual WASM large-document search, including browser-shortcut cancellation.
import {writeFile} from 'node:fs/promises';
const label=process.argv[2]??'after',appUrl=process.argv[3]??'http://127.0.0.1:8081/json-lens/';
const source='[\n'+Array(8850).fill('  {\n    "id": 12345,\n    "name": "JSON Lens performance example",\n    "active": true,\n    "data": [1, 2, 3]\n  }').join(',\n')+'\n]';
const file='/private/tmp/json-lens-search-million.json';await writeFile(file,source);
const tabs=await (await fetch('http://127.0.0.1:9227/json/list')).json();
const ws=new WebSocket(tabs.find(tab=>tab.type==='page').webSocketDebuggerUrl);
await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject;});
let id=0;const pending=new Map(),logs=[],errors=[];
ws.onmessage=event=>{const msg=JSON.parse(event.data);if(msg.id){const p=pending.get(msg.id);pending.delete(msg.id);if(msg.error)p.reject(msg.error);else p.resolve(msg.result);}else if(msg.method==='Runtime.consoleAPICalled')logs.push(msg.params.args.map(arg=>arg.value??arg.description??'').join(' '));else if(msg.method==='Runtime.exceptionThrown')errors.push(msg.params.exceptionDetails);};
 const call=(method,params={})=>new Promise((resolve,reject)=>{const request=++id;const timeout=setTimeout(()=>{pending.delete(request);reject(new Error('CDP timed out: '+method));},90000);pending.set(request,{resolve:value=>{clearTimeout(timeout);resolve(value)},reject:error=>{clearTimeout(timeout);reject(error)}});ws.send(JSON.stringify({id:request,method,params}));});
const evaluate=async expression=>{const r=await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw r.exceptionDetails;return r.result.value;};
const pause=ms=>evaluate(`new Promise(r=>setTimeout(()=>r(true),${ms}))`);
const click=async(x,y)=>{await call('Input.dispatchMouseEvent',{type:'mouseMoved',x,y});await pause(60);await call('Input.dispatchMouseEvent',{type:'mousePressed',x,y,button:'left',clickCount:1});await pause(60);await call('Input.dispatchMouseEvent',{type:'mouseReleased',x,y,button:'left',clickCount:1});await pause(150);};
const type=async letter=>{await call('Input.dispatchKeyEvent',{type:'keyDown',key:letter,code:'Key'+letter.toUpperCase(),text:letter,windowsVirtualKeyCode:letter.toUpperCase().charCodeAt(0)});await call('Input.dispatchKeyEvent',{type:'keyUp',key:letter,code:'Key'+letter.toUpperCase()});};
try {
 await call('Runtime.enable');await call('Page.enable');await call('Page.bringToFront');
 await call('Emulation.setDeviceMetricsOverride',{width:1280,height:800,deviceScaleFactor:1,mobile:false});
 const url=new URL(appUrl);url.searchParams.set('debug','1');url.searchParams.set('__probe',String(Date.now()));await call('Page.navigate',{url:url.href});
 if(!await evaluate(`(async()=>{for(let i=0;i<240;i++){if(location.href===${JSON.stringify(url.href)}&&window.wasmBindings&&!document.getElementById('loading'))return true;await new Promise(r=>setTimeout(r,250));}return false})()`))throw new Error('App failed to load');
 await pause(500);await click(300,300);
 await call('Input.dispatchKeyEvent',{type:'keyDown',key:'a',code:'KeyA',modifiers:2,windowsVirtualKeyCode:65});
 await call('Input.dispatchKeyEvent',{type:'keyUp',key:'a',code:'KeyA',modifiers:2});
 await evaluate(`(()=>{const data=new DataTransfer();data.setData('text/plain',${JSON.stringify(source)});document.dispatchEvent(new ClipboardEvent('paste',{clipboardData:data,bubbles:true,cancelable:true}))})()`);await pause(2000);
 await click(300,300);logs.length=0;
 // A cancellable DOM event measures preventDefault; untrusted events never open browser chrome.
 const cancelled=await evaluate(`(()=>{const e=new KeyboardEvent('keydown',{key:'f',code:'KeyF',metaKey:true,bubbles:true,cancelable:true});document.activeElement.dispatchEvent(e);return e.defaultPrevented})()`);
 await evaluate(`document.activeElement.dispatchEvent(new KeyboardEvent('keyup',{key:'f',code:'KeyF',metaKey:true,bubbles:true}));`);await pause(180);
 await evaluate(`(()=>{const p=window.searchProbe={running:true,gaps:[],last:performance.now()};function tick(now){if(!p.running)return;p.gaps.push(now-p.last);p.last=now;requestAnimationFrame(tick)}requestAnimationFrame(tick)})()`);
 for(const letter of 'Lens'){await type(letter);await pause(70);}
 await pause(600);
 // Search the same source again; the second request should use a cached source token.
 await type(' ');await pause(450);
 for(let i=0;i<12;i++){await call('Input.dispatchMouseEvent',{type:'mouseWheel',x:300,y:400,deltaX:0,deltaY:120});await pause(40);}
 const frames=await evaluate(`(()=>{const p=searchProbe;p.running=false;const gaps=p.gaps.filter(x=>x>=0).sort((a,b)=>a-b);return {median_ms:gaps[Math.floor(gaps.length/2)],p95_ms:gaps[Math.floor(gaps.length*.95)],max_ms:Math.max(...gaps),over_50_ms:gaps.filter(x=>x>50).length,frames:gaps.length}})()`);
 const result={label,url:appUrl,bytes:source.length,shortcut_default_prevented:cancelled,frames,build:await evaluate('({revision:document.documentElement.dataset.buildRevision,mode:document.documentElement.dataset.buildMode,simd:document.documentElement.dataset.wasmSimd})')};
 await writeFile(`/private/tmp/json-lens-search-${label}.json`,JSON.stringify(result,null,2)+'\n');
 await writeFile(`/private/tmp/json-lens-search-${label}.log`,logs.filter(line=>line.includes('[JSON Lens')).join('\n')+'\n');
 const shot=await call('Page.captureScreenshot',{format:'png'});await writeFile(`/private/tmp/json-lens-search-${label}.png`,Buffer.from(shot.data,'base64'));
 if(errors.length)throw new Error(JSON.stringify(errors));
 console.log(JSON.stringify(result,null,2));
}finally{ws.close();}
