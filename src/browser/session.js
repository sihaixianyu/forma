let database;
let restored = null;
let error = null;

export async function loadSession() {
    try {
        database = await new Promise((resolve, reject) => {
            const request = indexedDB.open('forma-session', 1);
            request.onupgradeneeded = () => request.result.createObjectStore('inputs');
            request.onsuccess = () => resolve(request.result);
            request.onerror = () => reject(request.error);
        });
        restored = await new Promise((resolve, reject) => {
            const request = database.transaction('inputs').objectStore('inputs').get('current');
            request.onsuccess = () => resolve(request.result ?? null);
            request.onerror = () => reject(request.error);
        });
    } catch (failure) {
        error = `Could not restore inputs: ${failure}`;
    }
}

export function restoredSession() { return restored; }

export function saveSession(left, right, compare) {
    if (!database) return;
    try {
        const transaction = database.transaction('inputs', 'readwrite');
        transaction.objectStore('inputs').put(JSON.stringify({left, right, compare}), 'current');
        transaction.onabort = () => { error = `Could not save inputs: ${transaction.error}`; };
        transaction.onerror = () => { error = `Could not save inputs: ${transaction.error}`; };
    } catch (failure) {
        error = `Could not save inputs: ${failure}`;
    }
}

export function sessionError() {
    const message = error;
    error = null;
    return message;
}
