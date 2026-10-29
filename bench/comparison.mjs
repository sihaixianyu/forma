import { performance } from 'node:perf_hooks';
import { createRequire } from 'node:module';
import path from 'node:path';

// Install jsonc-parser in a temporary directory; no npm dependencies are added to the app.
const require = createRequire(path.join(path.resolve(process.argv[2] ?? '.'), 'package.json'));
const { parse } = require('jsonc-parser');
const row = '{"id":123456,"name":"JSON Lens 中文 🚀","active":true,"values":[1,2,3,4]}';
let result;
function median(run) {
  for (let i = 0; i < 3; i++) result = run();
  const samples = [];
  for (let i = 0; i < 11; i++) {
    global.gc?.();
    const start = performance.now();
    result = run();
    samples.push(performance.now() - start);
  }
  return samples.sort((a, b) => a - b)[5];
}

console.log('input_mib,implementation,operation,median_ms');
for (const mib of [1, 5, 20]) {
  const count = Math.floor(mib * 1024 * 1024 / (Buffer.byteLength(row) + 1));
  const text = `[${Array(count).fill(row).join(',')}]`;
  for (const [name, parser] of [
    ['js-native', JSON.parse],
    ['jsonc-parser', text => parse(text, undefined, { disallowComments: true, allowTrailingComma: false })],
  ]) {
    for (const [operation, run] of [
      ['parse', () => parser(text)],
      ['parse_pretty', () => JSON.stringify(parser(text), null, 2)],
      ['parse_compact', () => JSON.stringify(parser(text))],
    ]) {
      console.log(`${mib},${name},${operation},${median(run).toFixed(3)}`);
    }
    const value = parser(text);
    // Serialization uses the same native stringify for both JS parser variants.
    if (name === 'js-native') {
      for (const [operation, run] of [
        ['cached_pretty', () => JSON.stringify(value, null, 2)],
        ['cached_compact', () => JSON.stringify(value)],
      ]) console.log(`${mib},${name},${operation},${median(run).toFixed(3)}`);
    }
  }
}
if (result === undefined) throw new Error('benchmark did not run');
