// Real browser Graph pan/zoom and card-drag checks with a million-character JSON.
import {writeFile} from 'node:fs/promises';
const label=process.argv[2]??'after';
const tabs=await (await fetch('http://127.0.0.1:9227/json/list')).json();
const ws=new WebSocket(tabs.find(tab=>tab.type==='page').webSocketDebuggerUrl);await new Promise(r=>ws.onopen=r);
let id=0;const pending=new Map(),logs=[];
ws.onmessage=({data})=>{const msg=JSON.parse(data);if(msg.id){const p=pending.get(msg.id);pending.delete(msg.id);msg.error?p.reject(msg.error):p.resolve(msg.result)}else if(msg.method==='Runtime.consoleAPICalled')logs.push(msg.params.args.map(a=>a.value??a.description??'').join(' '));else if(msg.method==='Runtime.exceptionThrown')logs.push('EXCEPTION '+JSON.stringify(msg.params.exceptionDetails))};
const call=(method,params={})=>new Promise((resolve,reject)=>{const n=++id;if(process.env.PROBE_VERBOSE)console.error(method);const timeout=setTimeout(()=>{pending.delete(n);reject(new Error('Browser request timed out: '+method))},60000);pending.set(n,{resolve:value=>{clearTimeout(timeout);resolve(value)},reject:error=>{clearTimeout(timeout);reject(error)}});ws.send(JSON.stringify({id:n,method,params}))});
const evaluate=async expression=>{const r=await call('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw r.exceptionDetails;return r.result.value};
const pause=ms=>evaluate(`new Promise(r=>setTimeout(r,${ms}))`);
const move=async(x,y)=>{await call('Input.dispatchMouseEvent',{type:'mouseMoved',x,y});await pause(50)};
const click=async(x,y)=>{await move(x,y);await call('Input.dispatchMouseEvent',{type:'mousePressed',x,y,button:'left',clickCount:1});await pause(50);await call('Input.dispatchMouseEvent',{type:'mouseReleased',x,y,button:'left',clickCount:1});await pause(150)};
const key=async(k,code,modifiers=0)=>{await call('Input.dispatchKeyEvent',{type:'keyDown',key:k,code,modifiers});await call('Input.dispatchKeyEvent',{type:'keyUp',key:k,code,modifiers})};
const paste=async text=>evaluate(`(()=>{const d=new DataTransfer();d.setData('text/plain',${JSON.stringify(text)});document.dispatchEvent(new ClipboardEvent('paste',{clipboardData:d,bubbles:true,cancelable:true}))})()`);
const probe=()=>evaluate(`(()=>{const p=window.graphProbe={running:true,last:performance.now(),gaps:[]};function tick(now){if(!p.running)return;p.gaps.push(now-p.last);p.last=now;requestAnimationFrame(tick)}requestAnimationFrame(tick)})()`);
const stop=()=>evaluate(`(()=>{const p=graphProbe;p.running=false;const gaps=p.gaps.filter(x=>x>=0).sort((a,b)=>a-b);return {frames:gaps.length,p95_ms:gaps[Math.floor(gaps.length*.95)],max_ms:Math.max(...gaps)}})()`);
const views=()=>logs.filter(line=>line.includes('event=graph_view')).length;
try{
 await call('Runtime.enable');await call('Page.enable');await call('Page.bringToFront');await call('Emulation.setDeviceMetricsOverride',{width:1280,height:800,deviceScaleFactor:1,mobile:false});
 const url='http://127.0.0.1:8081/json-lens/?debug=1&graph-probe='+Date.now();await call('Page.navigate',{url});
 await evaluate(`(async()=>{for(let i=0;i<120;i++){if(location.href===${JSON.stringify(url)}&&window.wasmBindings&&!document.getElementById('loading'))return;await new Promise(r=>setTimeout(r,250))}throw new Error('App did not start')})()`);await pause(350);
 const row='{"id":12345,"name":"JSON Lens performance example","active":true,"data":[1,2,3]}';
 const source='[\n'+Array(13000).fill(row).join(',\n')+'\n]';
 await click(300,300);await key('a','KeyA',2);await paste(source);await pause(1800);
 // Above the former 1 MiB cutoff, a normal edit must still update the graph.
 const editStart=logs.length;
 await click(300,300);await key('a','KeyA',2);await key('ArrowRight','ArrowRight');
 await call('Input.insertText',{text:' '});await pause(900);
 const graph_edit_updated=logs.slice(editStart).some(line=>line.includes('event=graph_result nodes=10000'));
 // Select root explicitly, then wheel over its visible card.
 await click(800,125);await key('a','KeyA',2);await paste('/0');await pause(200);await click(1228,125);await pause(200);
 const loaded=await call('Page.captureScreenshot',{format:'png'});await writeFile(`/private/tmp/json-lens-graph-${label}-loaded.png`,Buffer.from(loaded.data,'base64'));
 await move(780,450);const before=views();await probe();
 for(let i=0;i<8;i++){await call('Input.dispatchMouseEvent',{type:'mouseWheel',x:780,y:450,deltaX:0,deltaY:80});await pause(40)}
 const wheel_views=views()-before,wheel_frames=await stop();
 await click(1228,125);await move(780,450);const zoomBefore=views();await probe();
 for(let i=0;i<5;i++){await call('Input.dispatchMouseEvent',{type:'mouseWheel',x:780,y:450,deltaX:0,deltaY:-50,modifiers:2});await pause(60)}
 const zoom_views=views()-zoomBefore,zoom_frames=await stop();
 await click(1228,125);await move(920,620);const dragBefore=views();
 await call('Input.dispatchMouseEvent',{type:'mousePressed',x:920,y:620,button:'left',clickCount:1});
 for(let i=1;i<=8;i++){await call('Input.dispatchMouseEvent',{type:'mouseMoved',x:920+i*7,y:620-i*10,button:'left',buttons:1});await pause(35)}
 await call('Input.dispatchMouseEvent',{type:'mouseReleased',x:976,y:540,button:'left',clickCount:1});await pause(100);
 const drag_views=views()-dragBefore;
 const tailStart=logs.length;
 await click(800,125);await key('a','KeyA',2);await paste('/12999/name');await pause(700);
 const tail_focused=logs.slice(tailStart).some(line=>/event=graph_view nodes=2 visible=[1-9]/.test(line));
 const tailShot=await call('Page.captureScreenshot',{format:'png'});await writeFile(`/private/tmp/json-lens-graph-${label}-tail.png`,Buffer.from(tailShot.data,'base64'));
 const rootStart=logs.length;await click(1228,125);await pause(650);
 const root_restored=logs.slice(rootStart).some(line=>/event=graph_view nodes=10000 visible=[1-9]/.test(line));
 const sourceStart=logs.length;await click(270,741);await pause(700);
 const source_focused=logs.slice(sourceStart).some(line=>/event=graph_view nodes=2 visible=[1-9]/.test(line));
 const empty_views=logs.filter(line=>/event=graph_view nodes=\d+ visible=0\b/.test(line)).length;
 const result={label,bytes:source.length,graph_edit_updated,wheel_views,zoom_views,drag_views,tail_focused,root_restored,source_focused,empty_views,wheel_frames,zoom_frames};
 await writeFile(`/private/tmp/json-lens-graph-${label}.json`,JSON.stringify(result,null,2)+'\n');
 await writeFile(`/private/tmp/json-lens-graph-${label}.log`,logs.join('\n')+'\n');
 const shot=await call('Page.captureScreenshot',{format:'png'});await writeFile(`/private/tmp/json-lens-graph-${label}.png`,Buffer.from(shot.data,'base64'));
 console.log(JSON.stringify(result,null,2));
 if(label!=='before'&&(!graph_edit_updated||wheel_views===0||zoom_views===0||drag_views===0||!tail_focused||!root_restored||!source_focused||empty_views>0))throw new Error('Graph stopped updating, lost navigation or entered an empty viewport');
 if(logs.some(line=>line.startsWith('EXCEPTION')))throw new Error('Browser exception');
}finally{ws.close()}
