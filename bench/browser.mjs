// Run against a Trunk build with --features bench-web and Chrome remote debugging.
import { writeFile } from 'node:fs/promises';
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
  const browser = await call('Browser.getVersion');
  console.log(`Browser: ${browser.product}`);
  await writeFile(`${output}-environment.json`, JSON.stringify(browser, null, 2)+'\n');
  await call('Runtime.enable'); await call('Page.enable');
  await call('Emulation.setDeviceMetricsOverride', { width: 1280, height: 800, deviceScaleFactor: 1, mobile: false });
  const url = new URL(appUrl); url.searchParams.set('debug', '1');
  await call('Page.navigate', { url: url.href });
  const ready = await evaluate(`(async()=>{for(let i=0;i<120;i++){if(window.wasmBindings?.benchmark_json&&!document.getElementById('loading'))return true;await new Promise(r=>setTimeout(r,250));}return false;})()`);
  if (!ready) throw new Error('Build the app with trunk build --release --features bench-web');
  const samples = [
    String.raw`{"z":123456789012345678901234567890,"a":[-0,-0.0,1.2300e+42,1e9999,1e-9999],"z":999999999999999999999999999999}`,
    String.raw`{"中文":"🚀","escaped":"\u4e2d\ud83d\ude80\n\t\\","quote":"\"","empty":[],"null":null,"bool":true}`,
    '[1,]', '{} {}', '01', '[NaN]', String.raw`"\ud800"`, '{"x":}'
  ];
  const verified = await evaluate(`(()=>{
    const pad = ' '.repeat(32768);
    const samples = ${JSON.stringify(samples)};
    samples.forEach((sample,index)=>{
      const result = window.wasmBindings.verify_json(pad+sample);
      if((result==='invalid') !== (index>=2)) throw new Error('Unexpected validity for sample '+index);
    });
    for(const depth of [126,127,128,256]) window.wasmBindings.verify_json(pad+'['.repeat(depth)+'0'+']'.repeat(depth));
    return samples.length+4;
  })()`);
  console.log(`Browser parity checks: ${verified} passed`);
  let csv = 'input_mib,implementation,operation,median_ms\n';
  for (const mib of [1, 5, 20]) {
    await evaluate(`(()=>{const row='{"id":123456,"name":"JSON Lens 中文 🚀","active":true,"values":[1,2,3,4]}';window.benchInput='['+Array(Math.floor(${mib}*1048576/(new TextEncoder().encode(row).length+1))).fill(row).join(',')+']';})()`);
    for (const parser of ['serde', 'sonic']) {
      const results = JSON.parse(await evaluate(`window.wasmBindings.benchmark_json(window.benchInput, '${parser}')`));
      for (const [operation, ms] of Object.entries(results)) csv += `${mib},wasm-${parser},${operation},${ms.toFixed(3)}\n`;
      console.log(`${mib} MiB ${parser}: ${JSON.stringify(results)}`);
    }
    await evaluate('delete window.benchInput');
  }
  // Exercise a real queued editor job with DEBUG logging enabled.
  await call('Input.dispatchMouseEvent', { type: 'mousePressed', x: 300, y: 300, button: 'left', clickCount: 1 });
  await call('Input.dispatchMouseEvent', { type: 'mouseReleased', x: 300, y: 300, button: 'left', clickCount: 1 });
  await call('Input.dispatchKeyEvent', { type: 'keyDown', key: 'a', code: 'KeyA', modifiers: 2, windowsVirtualKeyCode: 65 });
  await call('Input.dispatchKeyEvent', { type: 'keyUp', key: 'a', code: 'KeyA', modifiers: 2 });
  await evaluate(`(()=>{const d=new DataTransfer();d.setData('text/plain',JSON.stringify({message:'中文 wrapping example '.repeat(2200),encoded:'5Lit5paH'}));document.dispatchEvent(new ClipboardEvent('paste',{clipboardData:d,bubbles:true,cancelable:true}));})()`);
  await evaluate('new Promise(resolve=>setTimeout(resolve,1500))');
  if (!logs.some(line => line.includes('parser=sonic') && /job=[1-9]/.test(line))) throw new Error('Real editor job did not use Sonic');
  if (!logs.some(line => line.includes('queue_ms='))) throw new Error('Queue DEBUG log missing');
  // Verify automatic Compare uses Sonic and emits both comparison stages.
  for (const [x, y] of [[780, 84], [900, 300]]) {
    await call('Input.dispatchMouseEvent', { type: 'mousePressed', x, y, button: 'left', clickCount: 1 });
    await call('Input.dispatchMouseEvent', { type: 'mouseReleased', x, y, button: 'left', clickCount: 1 });
  }
  await call('Input.dispatchKeyEvent', { type: 'keyDown', key: 'a', code: 'KeyA', modifiers: 2, windowsVirtualKeyCode: 65 });
  await call('Input.dispatchKeyEvent', { type: 'keyUp', key: 'a', code: 'KeyA', modifiers: 2 });
  await evaluate(`(()=>{const d=new DataTransfer();d.setData('text/plain',JSON.stringify({message:'中文 wrapping example '.repeat(2200),encoded:'SGVsbG8='}));document.dispatchEvent(new ClipboardEvent('paste',{clipboardData:d,bubbles:true,cancelable:true}));})()`);
  await evaluate('new Promise(resolve=>setTimeout(resolve,1800))');
  if (!logs.some(line => line.includes('stage=compare_alignment'))) throw new Error('Compare stage missing');
  if (!logs.some(line => line.includes('event=compare_result differences=1'))) throw new Error('Automatic comparison incorrect');
  if (errors.length) throw new Error(JSON.stringify(errors));
  await writeFile(`${output}.csv`, csv);
  await writeFile(`${output}.log`, logs.filter(line => line.includes('[JSON Lens')).join('\n')+'\n');
  console.log(`Saved ${output}.csv and ${output}.log`);
} finally { ws.close(); }
