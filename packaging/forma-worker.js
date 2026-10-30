let bindings;
// Forward Worker diagnostics to the page Console, retaining job IDs and realm.
console.debug = (...args) => self.postMessage({type: 'debug', text: args.join(' ')});
self.onmessage = async ({data}) => {
  if (data.type === 'init') {
    try {
      bindings = await import(data.moduleUrl);
      await bindings.default({module_or_path: data.wasmUrl});
      bindings.worker_configure(data.debug);
      self.postMessage({type: 'ready'});
    } catch (error) {
      self.postMessage({type: 'fatal', error: String(error)});
    }
    return;
  }
  if (data.type !== 'run') return;
  try {
    const payload = bindings.worker_dispatch(data.kind, data.payload, data.id);
    self.postMessage({type: 'result', id: data.id, payload}, [payload.buffer]);
  } catch (error) {
    self.postMessage({type: 'error', id: data.id, error: String(error)});
  }
};
