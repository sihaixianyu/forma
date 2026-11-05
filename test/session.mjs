import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import vm from 'node:vm';

const source = await readFile(new URL('../src/browser/session.js', import.meta.url), 'utf8');
const script = source.replaceAll('export ', '') + '\n globalThis.session = {loadSession, restoredSession, saveSession, sessionError};';
function page(records, fail = false) {
    const database = {
        createObjectStore() {},
        transaction() {
            const transaction = { error: 'quota exceeded', objectStore() { return {
                get(key) {
                    const request = {};
                    queueMicrotask(() => { request.result = records.get(key); request.onsuccess(); });
                    return request;
                },
                put(value, key) {
                    if (fail) queueMicrotask(() => transaction.onabort());
                    else records.set(key, value);
                },
            }; } };
            return transaction;
        },
    };
    const context = vm.createContext({queueMicrotask, indexedDB: {open() {
        const request = {};
        queueMicrotask(() => { request.result = database; request.onupgradeneeded(); request.onsuccess(); });
        return request;
    }}});
    vm.runInContext(script, context);
    return context.session;
}

test('refresh restores both inputs including invalid JSON, Unicode, and empty text', async () => {
    const records = new Map();
    const first = page(records);
    await first.loadSession();
    assert.equal(first.restoredSession(), null);
    first.saveSession('{"中文":', '', true);
    const next = page(records);
    await next.loadSession();
    assert.deepEqual(JSON.parse(next.restoredSession()), {left:'{"中文":', right:'', compare:true});
    next.saveSession('', '[1,2]', false);
    const final = page(records);
    await final.loadSession();
    assert.deepEqual(JSON.parse(final.restoredSession()), {left:'', right:'[1,2]', compare:false});
});

test('storage failures are surfaced and preserve the previous saved input', async () => {
    const records = new Map([['current', '{"left":"saved","right":"","compare":false}']]);
    const current = page(records, true);
    await current.loadSession();
    current.saveSession('new', 'new', true);
    await Promise.resolve();
    assert.match(current.sessionError(), /Could not save inputs/);
    assert.equal(current.sessionError(), null);
    assert.equal(JSON.parse(records.get('current')).left, 'saved');
});
