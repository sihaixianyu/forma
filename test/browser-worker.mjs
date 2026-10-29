// Run against a Trunk build with --features bench-web and Chrome remote debugging.
import { writeFile, readFile, mkdir } from 'node:fs/promises';
const debugUrl = process.argv[2] ?? 'http://127.0.0.1:9227';
const appUrl = process.argv[3] ?? 'http://127.0.0.1:8081/json-lens/';
const output = process.argv[4] ?? '/tmp/json-lens-browser';
const tabs = await (await fetch(`${debugUrl}/json/list`)).json();
const page = tabs.find(tab => tab.type === 'page');
if (!page) throw new Error('No Chrome page available');
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
let next = 0;
const pending = new Map(), logs = [], errors = [];
ws.onmessage = event => {
  const msg = JSON.parse(event.data);
  if (msg.id) {
    const request = pending.get(msg.id); pending.delete(msg.id);
    if (msg.error) request.reject(msg.error); else request.resolve(msg.result);
  } else if (msg.method === 'Runtime.consoleAPICalled') {
    logs.push(msg.params.args.map(arg => arg.value ?? arg.description ?? '').join(' '));
  } else if (msg.method === 'Runtime.exceptionThrown') errors.push(msg.params.exceptionDetails);
};
const call = (method, params = {}) => new Promise((resolve, reject) => {
  const id = ++next; pending.set(id, { resolve, reject }); ws.send(JSON.stringify({ id, method, params }));
});
const evaluate = async expression => {
  const result = await call('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true });
  if (result.exceptionDetails) throw new Error(JSON.stringify(result.exceptionDetails));
  return result.result.value;
};
try {
  await call('Runtime.enable');await call('Page.enable');
  await call('Emulation.setDeviceMetricsOverride',{width:1280,height:800,deviceScaleFactor:1,mobile:false});
  await call('Page.navigate',{url:appUrl+'?debug=1'});
  const ready=await evaluate(`(async()=>{for(let i=0;i<120;i++){if(window.wasmBindings&&!document.getElementById('loading'))return true;await new Promise(r=>setTimeout(r,250));}return false;})()`);
  if(!ready)throw new Error('App failed to load');
  const result=await evaluate(`(async()=>{
    const w=new Worker(window.jsonLensWorkerConfig.workerUrl,{type:'module'});
    const messages=[],waiting=new Map();let seq=500;
    let ready;const started=new Promise(resolve=>ready=resolve);
    w.onmessage=({data})=>{if(data.type==='ready')ready();else if(data.type==='debug')messages.push(data.text);else{const resolve=waiting.get(data.id);if(resolve){waiting.delete(data.id);resolve(data);}}};
    w.postMessage({...window.jsonLensWorkerConfig,type:'init'});await started;
    const request=async(kind,payload)=>{const id=++seq;const bytes=new TextEncoder().encode(JSON.stringify(payload));const p=new Promise(resolve=>waiting.set(id,resolve));w.postMessage({type:'run',kind,id,payload:bytes},[bytes.buffer]);const result=await p;if(result.type!=='result')throw new Error(result.error);return JSON.parse(new TextDecoder().decode(result.payload));};
    const work=job=>request('work',{job,revision:1,user_action:true});
    const unwrap=r=>{if(r.result.Err)throw new Error(r.result.Err);return r.result.Ok;};
    try{
      const input='{"z":123456789012345678901234567890,"a":[-0,1.2300e+42],"encoded":"5Lit5paH"}';
      const first=unwrap(await work({Process:[input,'Keep']}));
      let id=first.Document.id;
      const formatted=unwrap(await work({Transform:[{id,text:null},'Pretty']}));
      if(!formatted.Document.text.includes('123456789012345678901234567890')||!formatted.Document.text.includes('1.2300e+42')||!formatted.Document.text.includes('-0'))throw new Error('Number precision lost');
      if(formatted.Document.graph!==null)throw new Error('Cached format retransmitted graph');
      id=formatted.Document.id;
      const sorted=unwrap(await work({Transform:[{id,text:null},'SortAscending']}));
      if(sorted.Document.text.indexOf('"a"')>sorted.Document.text.indexOf('"z"'))throw new Error('Sort failed');
      id=sorted.Document.id;
      const pointer=await request('search',{Pointer:[1,{id,text:null},'/a/1']});if(pointer.Pointer[1]!==true)throw new Error('Pointer validation failed');
      const missing=await request('search',{Pointer:[2,{id,text:null},'/missing']});if(missing.Pointer[1]!==false)throw new Error('Invalid pointer accepted');
      const tree=await request('search',{Tree:[3,{id,text:null},'123456789012345678901234567890']});if(tree.Tree[1].total!==1)throw new Error('Tree search failed');
      const inspect=unwrap(await work({Inspect:[{id,text:null},'/z']}));if(inspect.Editing.json!=='123456789012345678901234567890')throw new Error('Inspect precision failed');
      const edited=unwrap(await work({Edit:[{id,text:null},'/z','999999999999999999999999999999',false]}));if(!edited.Document.text.includes('999999999999999999999999999999'))throw new Error('Edit failed');
      const renamed=unwrap(await work({Edit:[{id:edited.Document.id,text:null},'/z','renamed',true]}));
      if(!renamed.Document.text.includes('"renamed"')||renamed.Document.text.includes('"z"'))throw new Error('Property rename failed');
      const located=unwrap(await work({Locate:[{id:renamed.Document.id,text:null},'/renamed']}));if(!located.Located)throw new Error('Source location failed');
      const nested=unwrap(await work({Process:[JSON.stringify({nested:JSON.stringify({x:1})}),'Nested']}));if(!nested.Document.text.includes('"x"'))throw new Error('Nested expansion failed');
      const cmp=unwrap(await work({Compare:[input,input.replace('5Lit5paH','SGVsbG8=')]}));if(cmp.Comparison[0].length!==1)throw new Error('Compare failed');
      const invalid=unwrap(await work({Process:['{bad}','Keep']}));if(!invalid.Invalid)throw new Error('Invalid input accepted');
      const row='{"id":123456,"name":"JSON Lens 中文 🚀","active":true,"values":[1,2,3,4]}';
      const text='['+Array(Math.floor(20*1048576/(new TextEncoder().encode(row).length+1))).fill(row).join(',')+']';
      const payload=new TextEncoder().encode(JSON.stringify({job:{Process:[text,'Keep']},revision:2,user_action:true}));
      const idLarge=++seq;let ticks=0,last=performance.now(),maxGap=0;
      const timer=setInterval(()=>{const now=performance.now();maxGap=Math.max(maxGap,now-last);last=now;ticks++;},5);
      const start=performance.now();
      const p=new Promise(resolve=>waiting.set(idLarge,resolve));w.postMessage({type:'run',kind:'work',id:idLarge,payload},[payload.buffer]);
      const reply=await p;const roundtrip=performance.now()-start;clearInterval(timer);
      if(reply.type!=='result')throw new Error(reply.error);
      const doc=unwrap(JSON.parse(new TextDecoder().decode(reply.payload))).Document;
      if(!doc.graph.truncated||doc.graph.nodes.length!==10000)throw new Error('Graph limit failed');
      if(doc.text!==null)throw new Error('Unchanged source copied back');
      if(ticks<3)throw new Error('Main-thread heartbeat did not run during processing');
      let cacheMiss=false;
      try { await work({Transform:[{id:first.Document.id,text:null},'Pretty']}); } catch(error) { cacheMiss=error.message==='cache_miss'; }
      if(!cacheMiss)throw new Error('Evicted cache entry did not request hydration');
      const hydrated=unwrap(await work({Transform:[{id:first.Document.id,text:input},'Pretty']}));
      if(!hydrated.Document.text.includes('123456789012345678901234567890'))throw new Error('Cache hydration changed data');
      if(!messages.some(s=>s.includes('realm=worker')&&s.includes('parser=sonic')))throw new Error('Worker execution not logged');
      return {checks:16,input_mib:20,roundtrip_ms:roundtrip,heartbeat_ticks:ticks,max_heartbeat_gap_ms:maxGap,cross_origin_isolated:crossOriginIsolated,worker_logs:messages};
    }finally{w.terminate();}
  })()`);
  console.log(JSON.stringify({...result,worker_logs:undefined},null,2));
  await writeFile('/private/tmp/json-lens-worker-check.json',JSON.stringify(result,null,2));
  // Exercise the actual app channel, cached formatting and full-source saving.
  const downloadDir = `/private/tmp/json-lens-worker-downloads-${process.pid}`;
  await mkdir(downloadDir,{recursive:true});
  await call('Browser.setDownloadBehavior',{behavior:'allow',downloadPath:downloadDir});
  const pause=ms=>evaluate(`new Promise(r=>setTimeout(r,${ms}))`);
  const click=async(x,y)=>{await call('Input.dispatchMouseEvent',{type:'mousePressed',x,y,button:'left',clickCount:1});await call('Input.dispatchMouseEvent',{type:'mouseReleased',x,y,button:'left',clickCount:1});await pause(120);};
  const key=async(key,code,modifiers=0)=>{await call('Input.dispatchKeyEvent',{type:'keyDown',key,code,modifiers,windowsVirtualKeyCode:key.length===1?key.toUpperCase().charCodeAt(0):undefined});await call('Input.dispatchKeyEvent',{type:'keyUp',key,code,modifiers});};
  const paste=async text=>evaluate(`(()=>{const d=new DataTransfer();d.setData('text/plain',${JSON.stringify(text)});document.dispatchEvent(new ClipboardEvent('paste',{clipboardData:d,bubbles:true,cancelable:true}));})()`);
  const left='[{"z":123456789012345678901234567890,"a":"中文","encoded":"5Lit5paH"}]';
  await click(300,300);await key('a','KeyA',2);await paste(left);await pause(700);
  await key('s','KeyS',2);await pause(600);
  if(!logs.some(line=>line.includes('event=graph_cache reused=true')))throw new Error('App format cache missed');
  await call('Browser.grantPermissions',{origin:new URL(appUrl).origin,permissions:['clipboardReadWrite','clipboardSanitizedWrite']});
  const copyEditor=async()=>{
    await key('a','KeyA',2);await pause(30);
    await evaluate(`document.dispatchEvent(new ClipboardEvent('copy',{bubbles:true,cancelable:true}))`);
    await pause(50);return evaluate('navigator.clipboard.readText()');
  };
  await key('z','KeyZ',2);await pause(80);
  const undone=await copyEditor();if(undone!==left)throw new Error('Undo did not revert formatting: '+JSON.stringify(undone.slice(0,180)));
  await key('z','KeyZ',10);await pause(80);
  if(!(await copyEditor()).includes('\n'))throw new Error('Ctrl+Shift+Z did not redo formatting');
  await key('z','KeyZ',2);await pause(60);await key('y','KeyY',2);await pause(60);
  if(!(await copyEditor()).includes('\n'))throw new Error('Ctrl+Y did not redo formatting');
  await paste('{}');await pause(100);await key('ArrowLeft','ArrowLeft');await key('ArrowLeft','ArrowLeft');await key('Tab','Tab');await pause(80);
  if(await copyEditor()!=='  {}')throw new Error('Tab did not insert exactly two spaces');
  await key('Tab','Tab',8);await pause(80);
  if(await copyEditor()!=='{}')throw new Error('Shift+Tab did not remove two spaces');
  await key('z','KeyZ',2);await pause(80);
  if(await copyEditor()!=='  {}')throw new Error('Outdent was not undoable');
  await paste(left);await pause(700);await key('s','KeyS',2);await pause(500);
  await click(1140,25);await pause(350);
  const saved=await readFile(`${downloadDir}/Untitled.json`,'utf8');
  if(!saved.includes('123456789012345678901234567890')||!saved.includes('\n'))throw new Error('App save or precision failed');
  await click(1225,125);await pause(150);await click(780,84);await click(900,300);await paste(left.replace('5Lit5paH','SGVsbG8='));await pause(1000);
  if(!logs.some(line=>line.includes('realm=worker')&&line.includes('event=compare_result differences=1')))throw new Error('App automatic comparison failed');
  await key('s','KeyS',2);await pause(500);
  if(!(await copyEditor()).includes('\n'))throw new Error('Format shortcut did not target right editor');
  await key('z','KeyZ',2);await pause(80);
  if((await copyEditor()).includes('\n'))throw new Error('Right editor formatting was not undoable');
  await key('y','KeyY',2);await pause(80);
  if(!(await copyEditor()).includes('\n'))throw new Error('Right editor redo failed');
  await key('f','KeyF',2);await pause(120);await paste('中文');await pause(500);await key('f','KeyF',2);
  if(!logs.some(line=>line.includes('realm=worker')&&line.includes('stage=search_text')))throw new Error('App source search did not use Worker');
  if(logs.some(line=>line.includes('realm=main')&&/stage=(parse_sonic|parse_serde|graph_build|compare_walk|search_tree)/.test(line)))throw new Error('CPU processing leaked onto main thread');
  if(errors.length)throw new Error(JSON.stringify(errors));
  await writeFile('/private/tmp/json-lens-worker-ui.log',logs.filter(line=>line.includes('[JSON Lens')).join('\n')+'\n');
  console.log('App editor, cached Format, exact-number download, automatic Compare and search passed');
}finally{await writeFile('/private/tmp/json-lens-worker-ui.log',logs.filter(line=>line.includes('[JSON Lens')).join('\n')+'\n');ws.close();}
