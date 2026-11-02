// Run against a local Forma server and a dedicated Chrome CDP instance.
// Usage: node test/browser-page-scroll.mjs [http://127.0.0.1:8080] [9236]
import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';

const url = process.argv[2] ?? 'http://127.0.0.1:8080';
const port = process.argv[3] ?? '9236';
const tabs = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const socket = new WebSocket(tabs.find(tab => tab.type === 'page').webSocketDebuggerUrl);
await new Promise((resolve, reject) => { socket.onopen = resolve; socket.onerror = reject; });
let sequence = 0;
const pending = new Map();
socket.onmessage = event => {
  const message = JSON.parse(event.data);
  if (!message.id) return;
  const request = pending.get(message.id);
  pending.delete(message.id);
  if (message.error) request.reject(message.error);
  else request.resolve(message.result);
};
const call = (method, params = {}) => new Promise((resolve, reject) => {
  const id = ++sequence;
  pending.set(id, {resolve, reject});
  socket.send(JSON.stringify({id, method, params}));
});
const evaluate = async expression => {
  const result = await call('Runtime.evaluate', {expression, returnByValue: true, awaitPromise: true});
  if (result.exceptionDetails) throw result.exceptionDetails;
  return result.result.value;
};
const pause = ms => evaluate(`new Promise(resolve => setTimeout(() => resolve(true), ${ms}))`);
const viewport = async () => evaluate(`(() => {
  const canvas = document.getElementById('forma');
  return {x: scrollX, y: scrollY, bodyY: document.body.scrollTop,
    rootY: document.documentElement.scrollTop, width: innerWidth, height: innerHeight,
    canvas: canvas.getBoundingClientRect().toJSON()};
})()`);
const assertFixed = state => {
  assert.equal(state.x, 0);
  assert.equal(state.y, 0);
  assert.equal(state.bodyY, 0);
  assert.equal(state.rootY, 0);
  assert.equal(state.canvas.top, 0);
  assert.equal(state.canvas.left, 0);
  assert.equal(state.canvas.bottom, state.height);
  assert.equal(state.canvas.right, state.width);
};
const click = async (x, y) => {
  await call('Input.dispatchMouseEvent', {type: 'mousePressed', x, y, button: 'left', clickCount: 1});
  await call('Input.dispatchMouseEvent', {type: 'mouseReleased', x, y, button: 'left', clickCount: 1});
};
try {
  await call('Runtime.enable');
  await call('Page.enable');
  await call('Emulation.setDeviceMetricsOverride', {width: 1280, height: 800, deviceScaleFactor: 2, mobile: false});
  await call('Page.navigate', {url});
  assert.equal(await evaluate(`(async () => {
    for (let i = 0; i < 120; i++) {
      if (!document.getElementById('loading') && document.querySelector('input')) return true;
      await new Promise(resolve => setTimeout(resolve, 250));
    }
    return false;
  })()`), true, 'Forma must start');
  assertFixed(await viewport());

  // overflow:hidden allows focus to scroll the root to this offscreen IME input.
  // Check synchronously: a later canvas focus can hide this transient failure.
  assert.equal(await evaluate(`(() => {
    const input = document.querySelector('input');
    const oldStyle = input.getAttribute('style');
    const failures = [];
    for (const top of [900, 10000, 10000000, -10000000]) {
      input.blur(); input.style.top = top + 'px'; input.focus();
      const rect = document.getElementById('forma').getBoundingClientRect();
      if (scrollY !== 0 || rect.top !== 0 || rect.bottom !== innerHeight) failures.push({top,scrollY,rect:rect.toJSON()});
    }
    input.setAttribute('style', oldStyle);
    document.getElementById('forma').focus();
    return failures;
  })()`).then(failures => failures.length), 0, 'offscreen IME focus must not move the page');

  await click(200, 200);
  await pause(100);
  await call('Input.dispatchKeyEvent', {type: 'rawKeyDown', key: 'a', code: 'KeyA', modifiers: 4, windowsVirtualKeyCode: 65});
  await call('Input.dispatchKeyEvent', {type: 'keyUp', key: 'a', code: 'KeyA', modifiers: 4, windowsVirtualKeyCode: 65});
  const source = '[\n' + Array(50_000).fill('  {"id":123,"value":"large document scrolling","encoded":"{\\"name\\":\\"中文\\"}"}').join(',\n') + '\n]';
  await call('Input.insertText', {text: source});
  await pause(3000);
  assertFixed(await viewport());
  await call('Input.dispatchMouseEvent', {type: 'mousePressed', x: 636, y: 70, button: 'left', clickCount: 1});
  for (let i = 0; i < 30; i++) {
    await call('Input.dispatchMouseEvent', {type: 'mouseMoved', x: 636, y: 80 + (i * 173 % 680), button: 'left', buttons: 1});
    assertFixed(await viewport());
    await pause(20);
  }
  await call('Input.dispatchMouseEvent', {type: 'mouseReleased', x: 636, y: 600, button: 'left', clickCount: 1});
  await click(230, 500);
  await pause(100);
  assertFixed(await viewport());
  await call('Input.insertText', {text: ' '});
  await pause(100);
  assertFixed(await viewport());
  const screenshot = await call('Page.captureScreenshot', {format: 'png'});
  await writeFile('/private/tmp/forma-browser-page-scroll-fixed.png', Buffer.from(screenshot.data, 'base64'));
  console.log(JSON.stringify({passed: true, bytes: source.length, viewport: await viewport()}, null, 2));
} finally {
  socket.close();
}
