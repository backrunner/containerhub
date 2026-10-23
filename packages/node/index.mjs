import { spawn } from 'node:child_process';
import { randomBytes, randomUUID } from 'node:crypto';
import { isAbsolute } from 'node:path';

const maximum = 1024 * 1024;
const active = new Set(['Submitting', 'Queued', 'Running', 'Reconciling']);
const decode = new TextDecoder('utf-8', { fatal: true });
export class ContainerHubError extends Error {
  constructor(kind, message, operationId) { super(message); this.name = 'ContainerHubError'; this.kind = kind; this.operationId = operationId; }
}

/** One Rust child process, one isolated state directory; no shell and no listening TCP port. */
export class ContainerHub {
  #child; #pending = new Map(); #buffer = Buffer.alloc(0); #closed = false; #closing = false; #exit;
  static async connect(options) {
    const client = new ContainerHub(options);
    try { const hello = await client.#request({ method: 'hello' });
      if (hello.protocol !== 1 || hello.transport !== 'stdio') throw new ContainerHubError('Unsupported', 'Unsupported ContainerHub bridge');
      return client;
    } catch (error) { client.#child.kill(); throw error; }
  }
  constructor({ binary, stateDirectory, mode, profile, agent, appleBinary, dockerContext, dockerBinary }) {
    if (!isAbsolute(binary) || !isAbsolute(stateDirectory)) throw new TypeError('binary and stateDirectory must be absolute paths');
    if (!['apple', 'docker', 'demo', 'remote'].includes(mode)) throw new TypeError('Choose apple, docker, demo or remote mode explicitly');
    if (mode === 'remote' && (!profile || !isAbsolute(profile))) throw new TypeError('Remote mode requires an absolute profile path');
    if (mode !== 'remote' && (profile || agent)) throw new TypeError('Remote options require remote mode');
    if (appleBinary !== undefined && (mode !== 'apple' || !isAbsolute(appleBinary))) throw new TypeError('appleBinary requires apple mode and an absolute path');
    if (mode === 'docker' && (typeof dockerContext !== 'string' || !/^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,127}$/.test(dockerContext))) throw new TypeError('Docker mode requires a valid dockerContext');
    if (mode !== 'docker' && (dockerContext !== undefined || dockerBinary !== undefined)) throw new TypeError('Docker options require docker mode');
    if (dockerBinary !== undefined && !isAbsolute(dockerBinary)) throw new TypeError('dockerBinary must be an absolute path');
    const args = ['--state', stateDirectory];
    if (mode === 'docker') { args.push('--docker-context', dockerContext); if (dockerBinary) args.push('--docker-binary', dockerBinary); }
    if (appleBinary) args.push('--apple-binary', appleBinary);
    if (mode === 'demo') args.push('--demo');
    if (mode === 'remote') { args.push('--remote-profile', profile); if (agent) args.push('--remote-agent', agent); }
    this.#child = spawn(binary, args, { shell: false, stdio: ['pipe', 'pipe', 'pipe'] });
    let diagnostics = '';
    this.#child.stderr.on('data', bytes => { diagnostics = (diagnostics + bytes.toString('utf8')).slice(-4096); });
    this.#child.stdout.on('data', bytes => {
      try {
        this.#buffer = Buffer.concat([this.#buffer, bytes]);
        for (;;) {
          const end = this.#buffer.indexOf(10); if (end < 0) break;
          if (end > maximum) throw new Error('Oversized bridge response');
          const message = JSON.parse(decode.decode(this.#buffer.subarray(0, end)));
          this.#buffer = this.#buffer.subarray(end + 1);
          if (message.version !== 1 || typeof message.id !== 'string') throw new Error('Invalid bridge envelope');
          const pending = this.#pending.get(message.id); if (!pending) continue; // A read may already have timed out.
          this.#pending.delete(message.id); clearTimeout(pending.timer);
          if (message.error) pending.reject(new ContainerHubError(message.error.kind, message.error.message, pending.operationId));
          else pending.resolve(message.result);
        }
        if (this.#buffer.length > maximum) throw new Error('Oversized bridge response');
      } catch (error) { this.#fail(error); this.#child.kill(); }
    });
    this.#child.stdin.on('error', error => this.#fail(error));
    this.#child.on('error', error => this.#fail(error));
    this.#exit = new Promise(resolve => this.#child.once('close', (code, signal) => {
      this.#closed = true;
      this.#fail(new Error(`ContainerHub bridge closed (${signal ?? code})${diagnostics ? `: ${diagnostics.trim()}` : ''}`));
      resolve({ code, signal });
    }));
  }
  #fail(error) {
    this.#closed = true;
    for (const pending of this.#pending.values()) {
      clearTimeout(pending.timer);
      pending.reject(new ContainerHubError(pending.operationId ? 'UnknownOutcome' : 'Unavailable', error.message, pending.operationId));
    }
    this.#pending.clear();
  }
  #request(request, operationId) {
    if (this.#closed || this.#closing) return Promise.reject(new ContainerHubError('Unavailable', 'ContainerHub client is closed', operationId));
    if (this.#pending.size >= 32) return Promise.reject(new ContainerHubError('Conflict', 'At most 32 concurrent requests are allowed', operationId));
    const id = randomUUID(), frame = Buffer.from(JSON.stringify({ version: 1, id, request }) + '\n');
    if (frame.length > maximum) { frame.fill(0); return Promise.reject(new ContainerHubError('InvalidData', 'Request exceeds 1 MiB', operationId)); }
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.#pending.delete(id);
        reject(new ContainerHubError(operationId ? 'UnknownOutcome' : 'Timeout', 'Bridge response deadline exceeded; query the original operation ID before retrying', operationId));
      }, 15_000);
      this.#pending.set(id, { resolve, reject, timer, operationId });
      this.#child.stdin.write(frame, error => { frame.fill(0); if (error) this.#fail(error); });
    });
  }
  hostSnapshot() { return this.#request({ method: 'host_snapshot' }); }
  diagnostics() { return this.#request({ method: 'diagnostics' }); }
  stacks() { return this.#request({ method: 'stacks' }); }
  stackCommands(environment, name, action, kind = 'compose') { return this.#request({ method: 'stack_commands', params: { environment, name, action, kind } }); }
  snapshot() { return this.#request({ method: 'snapshot' }); }
  resources(kind) { return this.#request({ method: 'resources', params: { kind } }); }
  inspectContainer(key) { return this.#request({ method: 'inspect_container', params: { key } }); }
  cancel(id) { return this.#request({ method: 'cancel', params: { id } }, id); }
  reconcile(id) { return this.#request({ method: 'reconcile', params: { id } }, id); }
  inspectImage(environment, reference) { return this.#request({ method: 'inspect_image', params: { environment, reference } }); }
  exportLogs(key, { source = 'Standard', query = '' } = {}) { return this.#request({ method: 'export_logs', params: { key, source, query } }); }
  logs(key, source = 'Standard') { return this.#request({ method: 'logs', params: { key, source } }); }
  stats(key) { return this.#request({ method: 'stats', params: { key } }); }
  operation(id) { return this.#request({ method: 'operation', params: { id } }, id); }
  operations() { return this.#request({ method: 'operations' }); }
  submit(command, { operationId = randomBytes(6).readUIntBE(0, 6) || 1, secrets = {} } = {}) {
    if (!Number.isSafeInteger(operationId) || operationId <= 0) return Promise.reject(new TypeError('operationId must be a positive safe integer'));
    return this.#request({ method: 'submit', params: { operation_id: operationId, command, secrets } }, operationId);
  }
  async wait(operationId, { timeoutMs = 15 * 60_000, signal, onProgress } = {}) {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      signal?.throwIfAborted();
      const operation = await this.operation(operationId);
      onProgress?.(operation);
      if (!active.has(operation.state)) return operation;
      if (Date.now() >= deadline) throw new ContainerHubError('UnknownOutcome', 'Operation is still pending; no command was resubmitted', operationId);
      await new Promise(resolve => setTimeout(resolve, 100));
    }
  }
  async execute(command, options = {}) { const operation = await this.submit(command, options); return this.wait(operation.id, options); }
  async close({ timeoutMs = 15_000 } = {}) {
    if (!this.#closed) {
      this.#closing = true; this.#child.stdin.end();
      let timer;
      const timedOut = new Promise(resolve => { timer = setTimeout(() => resolve(true), timeoutMs); });
      const forced = await Promise.race([this.#exit.then(() => false), timedOut]); clearTimeout(timer);
      if (forced) { this.#child.kill(); await this.#exit; throw new ContainerHubError('UnknownOutcome', 'Bridge shutdown timed out; unfinished mutations require reconciliation'); }
    }
    const { code, signal } = await this.#exit;
    if (code !== 0 || signal) throw new ContainerHubError('UnknownOutcome', 'Bridge exited abnormally; query unfinished operations after reconnecting');
  }
}
