// Browser-compatible transport. No owner private key is imported or required.
// The caller obtains owner/Agent pins and the signed grant through trusted device enrollment.
const text = new TextEncoder();
const decoder = new TextDecoder('utf-8', { fatal: true });
const maxRecord = 1024 * 1024;
function check(condition, message) { if (!condition) throw new Error(message); }
export function base64(bytes) {
  let value = '';
  for (const byte of bytes) value += String.fromCharCode(byte);
  return btoa(value).replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '');
}
function unbase64(value) {
  const raw = atob(value.replaceAll('-', '+').replaceAll('_', '/'));
  return Uint8Array.from(raw, c => c.charCodeAt(0));
}
function signingInput(domain, payload) {
  const prefix = text.encode('ContainerHub-Signature\0');
  const label = text.encode(domain);
  const result = new Uint8Array(prefix.length + 4 + label.length + 4 + payload.length);
  const view = new DataView(result.buffer);
  result.set(prefix); view.setUint32(prefix.length, label.length);
  result.set(label, prefix.length + 4);
  view.setUint32(prefix.length + 4 + label.length, payload.length);
  result.set(payload, prefix.length + 8 + label.length);
  return result;
}
export async function createDevice() {
  const pair = await crypto.subtle.generateKey({ name: 'ECDSA', namedCurve: 'P-256' }, false, ['sign', 'verify']);
  const publicKey = base64(new Uint8Array(await crypto.subtle.exportKey('raw', pair.publicKey)));
  return { publicKey, privateKey: pair.privateKey };
}
export async function loadBrowserDevice() {
  const database = await new Promise((resolve, reject) => {
    const request = indexedDB.open('containerhub-controller-v1', 1);
    request.onupgradeneeded = () => request.result.createObjectStore('identity');
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
  try {
    const existing = await new Promise((resolve, reject) => {
      const request = database.transaction('identity').objectStore('identity').get('device');
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => reject(request.error);
    });
    if (existing) return existing;
    const device = await createDevice();
    // add(), not put(): two tabs cannot silently replace each other's enrolled key.
    await new Promise((resolve, reject) => {
      const transaction = database.transaction('identity', 'readwrite');
      transaction.objectStore('identity').add(device, 'device');
      transaction.oncomplete = resolve;
      transaction.onerror = () => reject(transaction.error);
      transaction.onabort = () => reject(transaction.error ?? new Error('Device persistence aborted'));
    });
    return device;
  } finally { database.close(); }
}
async function sign(device, domain, value) {
  check(!device.privateKey.extractable, 'Browser device private key must be non-extractable');
  const payload = text.encode(JSON.stringify(value));
  const signature = await crypto.subtle.sign({ name: 'ECDSA', hash: 'SHA-256' }, device.privateKey, signingInput(domain, payload));
  return { payload: base64(payload), signature: base64(new Uint8Array(signature)) };
}
function openSocket(url) {
  return new Promise((resolve, reject) => {
    const parsed = new URL(url);
    const loopback = ['127.0.0.1', '[::1]'].includes(parsed.hostname);
    check(parsed.protocol === 'wss:' || (parsed.protocol === 'ws:' && loopback), 'A WSS endpoint is required');
    check(!parsed.username && !parsed.password && !parsed.hash, 'Invalid endpoint');
    const socket = new WebSocket(url);
    socket.binaryType = 'arraybuffer';
    const queue = [], waiters = [];
    let closed;
    const fail = error => { closed = error; for (const waiter of waiters.splice(0)) waiter.reject(error); };
    const timer = setTimeout(() => { socket.close(); reject(new Error('Connection timeout')); }, 10_000);
    socket.onopen = () => { clearTimeout(timer); resolve({
      send: bytes => { check(socket.bufferedAmount < 2 * maxRecord, 'WebSocket backpressure limit'); socket.send(bytes); },
      close: () => socket.close(),
      receive: () => new Promise((resolve, reject) => {
        if (closed) return reject(closed);
        if (queue.length) return resolve(queue.shift());
        const waiter = { resolve: value => { clearTimeout(deadline); resolve(value); }, reject: error => { clearTimeout(deadline); reject(error); } };
        const deadline = setTimeout(() => {
          const index = waiters.indexOf(waiter); if (index >= 0) waiters.splice(index, 1);
          reject(new Error('Response timeout')); socket.close();
        }, 15_000);
        waiters.push(waiter);
      }),
    }); };
    socket.onmessage = event => {
      if (!(event.data instanceof ArrayBuffer) || event.data.byteLength > 65_535 || queue.length >= 32) { fail(new Error('Invalid or excessive frames')); socket.close(); return; }
      const bytes = new Uint8Array(event.data);
      if (waiters.length) waiters.shift().resolve(bytes); else queue.push(bytes);
    };
    socket.onerror = () => { clearTimeout(timer); const error = new Error('WebSocket connection failed'); fail(error); reject(error); };
    socket.onclose = () => { clearTimeout(timer); const error = new Error('Connection closed'); fail(error); reject(error); };
  });
}

export async function connectRemote({ wasm, endpoint, agentPublicKey, ownerPublicKey, device, grant, policy }) {
  const verify = (key, domain, signed) => JSON.parse(wasm.verify_document(key, domain, JSON.stringify(signed)));
  const delegation = verify(ownerPublicKey, 'containerhub.device-grant.v1', grant);
  check(delegation.owner === ownerPublicKey && delegation.device === device.publicKey, 'Device grant mismatch');
  const socket = await openSocket(endpoint.url);
  const channel = new wasm.BrowserChannel();
  let failed = false;
  const close = () => { if (!failed) { failed = true; socket.close(); channel.free(); } };
  async function send(value) {
    check(!failed, 'Session closed');
    const bytes = channel.seal(JSON.stringify(value));
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    for (let offset = 0; offset < bytes.length;) {
      const length = view.getUint32(offset); offset += 4;
      socket.send(bytes.subarray(offset, offset + length)); offset += length;
    }
  }
  async function receive() {
    for (;;) {
      const message = channel.open(await socket.receive());
      if (message !== undefined) return JSON.parse(message);
    }
  }
  try {
    if (endpoint.mode === 'relay') {
      JSON.parse(decoder.decode(await socket.receive()));
      socket.send(text.encode(JSON.stringify({ role: 'controller', route: endpoint.route })));
      check(JSON.parse(decoder.decode(await socket.receive())).paired, 'Relay pairing failed');
    } else check(endpoint.mode === 'direct', 'Unknown transport');
    socket.send(channel.start());
    const binding = channel.finish(await socket.receive());
    const proof = verify(agentPublicKey, 'containerhub.agent-proof.v1', await receive());
    check(proof.version === 1 && proof.agent === agentPublicKey && proof.channel === binding, 'Agent identity or session mismatch');
    let localPolicy = verify(ownerPublicKey, 'containerhub.owner-policy.v1', policy);
    if (proof.policy) {
      const current = verify(ownerPublicKey, 'containerhub.owner-policy.v1', proof.policy);
      check(current.revision !== localPolicy.revision || proof.policy.payload === policy.payload, 'Conflicting owner policy');
      if (current.revision > localPolicy.revision) { policy = proof.policy; localPolicy = current; }
    }
    const now = Math.floor(Date.now() / 1000);
    check(localPolicy.owner === ownerPublicKey && localPolicy.expires_at > now, 'Owner policy expired');
    const claims = { version: 1, agent: agentPublicKey, channel: binding, claim: false, grant, policy, app_credential: null };
    await send(await sign(device, 'containerhub.client-proof.v1', claims));
    const receipt = verify(agentPublicKey, 'containerhub.binding-receipt.v1', await receive());
    check(receipt.agent === agentPublicKey && receipt.owner === ownerPublicKey, 'Binding receipt mismatch');
    let pending = Promise.resolve();
    return { receipt, close, request(value) {
      const result = pending.then(async () => {
        try { await send(value); return await receive(); } catch (error) { close(); throw error; }
      });
      pending = result.catch(() => {});
      return result;
    } };
  } catch (error) { close(); throw error; }
}
