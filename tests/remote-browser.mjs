// Actual WebCrypto + WASM + WebSocket interoperability, using an isolated FakeProvider.
// Run after building the development binaries and generating the Node-target WASM bindings.
import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile, rm, readdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { spawn, execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { createDevice, connectRemote } from '../packages/client-js/remote.mjs';
import { ContainerHub } from '../packages/node/index.mjs';

const binary = resolve('target/debug/containerhub-agent');
const fixture = resolve('target/debug/examples/development_fixture');
const wasm = createRequire(import.meta.url)(resolve('.local/browser-protocol/containerhub_remote.js'));
const root = await mkdtemp(join(tmpdir(), 'containerhub-browser-'));
const children = [];
function run(args) { return execFileSync(binary, args, { encoding: 'utf8', timeout: 20_000 }); }
async function service(args) {
  const child = spawn(binary, args, { stdio: ['ignore', 'pipe', 'pipe'] }); children.push(child);
  return await new Promise((resolve, reject) => {
    let stderr = '';
    const timer = setTimeout(() => reject(new Error('Service startup timeout')), 15_000);
    child.stderr.on('data', bytes => {
      stderr += bytes;
      const match = /listening on (127\.0\.0\.1:\d+)/.exec(stderr);
      if (match) { clearTimeout(timer); resolve(`ws://${match[1]}`); }
    });
    child.on('exit', code => { clearTimeout(timer); reject(new Error(`Service exited ${code}: ${stderr}`)); });
    child.on('error', error => { clearTimeout(timer); reject(error); });
  });
}
try {
  const state = join(root, 'fixture');
  execFileSync(fixture, [state], { stdio: ['ignore', 'ignore', 'pipe'] });
  const profile = join(state, 'controller');
  const relayURL = await service(['relay', '--listen', '127.0.0.1:0']);
  const directURL = await service(['serve', '--state', join(state, 'agent'), '--listen', '127.0.0.1:0', '--relay', relayURL]);
  const receipt = JSON.parse(run(['connect', '--profile', profile, '--url', directURL, '--claim']));
  const nodeOptions = { binary: resolve('target/debug/containerhub-bridge'), stateDirectory: join(root, 'node'), mode: 'remote', profile };
  let node = await ContainerHub.connect(nodeOptions);
  let originalCreation;
  try {
    const environment = (await node.snapshot()).environment;
    const diagnostics = await node.diagnostics();
    assert.equal(diagnostics.environment, environment);
    assert.equal(diagnostics.checks.length, 21);
    assert.equal(diagnostics.checks.find(c => c.feature === 'start_runtime').available, false);
    assert.ok(diagnostics.checks.filter(check => check.feature !== 'start_runtime').every(check => check.available));
    assert.equal((await node.execute({ kind: 'start_runtime', environment })).state, 'Failed');
    const reference = 'example.invalid/remote-fixture:demo';
    assert.equal((await node.execute({ kind: 'pull_image', environment, reference })).state, 'Succeeded');
    const created = await node.execute({ kind: 'create_container', environment, spec: { name: 'remote-sdk-fixture', image: reference, cpus: 1, memory_mib: 256, options: { start_after_create: true, user: '1000:1000', working_directory: '/app', read_only: true, secret_environment: ['API_TOKEN'], startup: { entrypoint: '/app', arguments: ['--mode', 'two words', ''] } } } }, { secrets: { API_TOKEN: 'node-remote-canary' } });
    assert.equal(created.state, 'Succeeded');
    assert.equal(created.result.value.state, 'Running');
    assert.equal(created.receipt.kind, 'remote');
    originalCreation = created;
    const key = created.result.value.key;
    assert.equal(key.environment, environment);
    const details = await node.inspectContainer(created.result.value.key);
    assert.equal(details.read_only, true);
    assert.ok(details.environment_names.includes('API_TOKEN'));
    assert.equal(details.argument_count, 3);
    assert.ok(!JSON.stringify([created, details]).includes('node-remote-canary'));
    assert.equal(details.working_directory, '/app');
    assert.equal(details.container.key.environment, environment);
    const image = await node.inspectImage(environment, reference);
    assert.equal(image.key.environment, environment);
    assert.deepEqual(image.used_by, [key]);
    assert.equal((await node.execute({ kind: 'delete_image', key: image.key })).state, 'Failed');
    const lifecycle = action => ({ kind: 'lifecycle', key, action });
    assert.equal((await node.execute(lifecycle('Start'))).state, 'Succeeded');
    assert.equal((await node.stats(key)).key.environment, environment);
    const exported = await node.exportLogs(key, { query: 'warn' });
    assert.equal(exported.line_count, 1);
    assert.ok(exported.content.includes(environment));
    assert.equal((await node.execute(lifecycle('Stop'))).state, 'Succeeded');
    assert.equal((await node.execute({ kind: 'delete_container', key })).state, 'Succeeded');
    assert.equal((await node.execute({ kind: 'delete_image', key: image.key })).state, 'Succeeded');
    await assert.rejects(node.inspectImage(environment, reference), { kind: 'NotFound' });
  } finally { await node.close(); }
  // Isolated crash-journal fixture: authoritative Agent completion exists, local final save was lost.
  const journalFile = (await readdir(nodeOptions.stateDirectory, { recursive: true })).find(name => name.endsWith('operations.json'));
  assert.ok(journalFile);
  const journalPath = join(nodeOptions.stateDirectory, journalFile);
  const records = JSON.parse(await readFile(journalPath, 'utf8'));
  const interrupted = records.find(op => op.id === originalCreation.id);
  interrupted.state = 'Running'; interrupted.result = null; interrupted.finished_at = null;
  await writeFile(journalPath, JSON.stringify(records), { mode: 0o600 });
  node = await ContainerHub.connect(nodeOptions);
  try {
    const restored = await node.wait(originalCreation.id);
    assert.equal(restored.state, 'Succeeded');
    assert.deepEqual(restored.receipt, originalCreation.receipt);
    assert.deepEqual(restored.result, originalCreation.result);
    assert.ok(!(await node.snapshot()).containers.some(c => c.key.id === 'remote-sdk-fixture'), 'recovering create must not recreate the later-deleted container');
    assert.equal((await node.reconcile(restored.id)).id, restored.id, 'terminal reconciliation is idempotent');
  } finally { await node.close(); }
  assert.ok(!(await readFile(journalPath, 'utf8')).includes('node-remote-canary'));
  for (const name of await readdir(join(state, 'agent'))) {
    const path = join(state, 'agent', name);
    try { assert.ok(!(await readFile(path)).includes(Buffer.from('node-remote-canary'))); } catch (error) { if(error.code !== 'EISDIR') throw error; }
  }
  const device = await createDevice();
  assert.equal(device.privateKey.extractable, false);
  await assert.rejects(crypto.subtle.exportKey('pkcs8', device.privateKey));
  const deviceFile = join(root, 'device-public.json'), bundleFile = join(root, 'grant.json');
  await writeFile(deviceFile, JSON.stringify(device.publicKey), { mode: 0o600 });
  run(['grant', '--profile', profile, '--device-public', deviceFile, '--output', bundleFile]);
  const bundle = JSON.parse(await readFile(bundleFile, 'utf8'));
  assert.equal(bundle.root_key, undefined);
  assert.equal(bundle.device_key, undefined);
  const agentId = `p256:${Buffer.from(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(receipt.agent))).toString('base64url')}`;
  const options = { wasm, agentPublicKey: receipt.agent, ownerPublicKey: bundle.owner, device, grant: bundle.grant, policy: bundle.policy };
  const direct = await connectRemote({ ...options, endpoint: { mode: 'direct', url: directURL } });
  const snapshot = await direct.request({ method: 'snapshot' });
  assert.equal(snapshot.type, 'snapshot');
  assert.equal(snapshot.data.lifecycle_supported, false);
  assert.equal(snapshot.data.capabilities.delete_image, false);
  assert.equal(snapshot.data.capabilities.inspect_image, true);
  assert.equal(snapshot.data.capabilities.cancel_pull, false);
  assert.equal(snapshot.data.capabilities.create_advanced, false);
  assert.equal(snapshot.data.capabilities.create_startup, false);
  assert.equal(snapshot.data.capabilities.create_and_start, false);
  const diagnostics = await direct.request({ method: 'diagnostics' });
  assert.equal(diagnostics.type, 'diagnostics');
  assert.equal(diagnostics.data.environment, snapshot.data.environment);
  assert.ok(diagnostics.data.checks.find(check => check.feature === 'containers').available);
  for (const feature of ['lifecycle', 'create_container', 'advanced_create', 'secret_environment', 'startup_command', 'create_and_start', 'pull_image', 'cancel_pull', 'delete_container', 'delete_image']) {
    const check = diagnostics.data.checks.find(check => check.feature === feature);
    assert.equal(check.available, false);
    assert.ok(check.reason.includes('授权'));
  }
  const inspected = await direct.request({ method: 'inspect_container', params: { key: snapshot.data.containers[0].key } });
  assert.equal(inspected.type, 'container_details');
  assert.deepEqual(inspected.data.container.key, snapshot.data.containers[0].key);
  assert.ok(snapshot.data.environment.startsWith(`agent:${agentId}:`));
  const key = snapshot.data.containers[0].key;
  const imageDetails = await direct.request({ method: 'inspect_image', params: { environment: snapshot.data.environment, reference: snapshot.data.containers[0].image } });
  assert.equal(imageDetails.type, 'image_details');
  assert.ok(imageDetails.data.used_by.every(key => key.environment === snapshot.data.environment));
  const logs = await direct.request({ method: 'logs', params: { key, source: 'Standard' } });
  assert.equal(logs.type, 'logs');
  assert.ok(logs.data.lines.length > 0);
  const denied = await direct.request({ method: 'lifecycle', params: { request_id: crypto.randomUUID(), key, action: 'Stop', expires_at: Math.floor(Date.now() / 1000) + 60 } });
  assert.equal(denied.type, 'error');
  assert.equal(denied.data.kind, 'PermissionDenied');
  for (const command of [
    { kind: 'pull_image', environment: snapshot.data.environment, reference: 'example.invalid/forbidden:v1' },
    { kind: 'delete_container', key },
    { kind: 'delete_image', key: imageDetails.data.key },
    { kind: 'create_container', environment: snapshot.data.environment, spec: { name: 'forbidden', image: 'example.invalid/forbidden:v1', cpus: 1, memory_mib: 256, options: { startup: { entrypoint: '/app', arguments: ['--check'] } } } },
  ]) {
    const result = await direct.request({ method: 'submit', params: { request_id: crypto.randomUUID(), command, expires_at: Math.floor(Date.now() / 1000) + 60 } });
    assert.equal(result.type, 'error'); assert.equal(result.data.kind, 'PermissionDenied');
  }
  const secretDenied = await direct.request({ method: 'submit_with_secrets', params: {
    request: { request_id: crypto.randomUUID(), expires_at: Math.floor(Date.now()/1000)+60,
      command: { kind:'create_container',environment:snapshot.data.environment,spec:{name:'forbidden-secret',image:'image:v1',cpus:1,memory_mib:256,options:{secret_environment:['API_TOKEN']}} } },
    secrets: { API_TOKEN: 'web-denied-canary' },
  }});
  assert.equal(secretDenied.type, 'error'); assert.equal(secretDenied.data.kind, 'PermissionDenied');
  direct.close();
  const relayed = await connectRemote({ ...options, endpoint: { mode: 'relay', url: relayURL, route: agentId } });
  assert.equal((await relayed.request({ method: 'snapshot' })).data.environment, snapshot.data.environment);
  // A newer signed policy is distributed through the pinned Agent handshake, without copying keys.
  const native = JSON.parse(await readFile(join(profile, 'profile.json'), 'utf8'));
  const deviceId = `p256:${Buffer.from(await crypto.subtle.digest('SHA-256', new TextEncoder().encode(device.publicKey))).toString('base64url')}`;
  run(['revoke', '--profile', profile, '--device', deviceId]);
  await new Promise(resolve => setTimeout(resolve, 1100));
  await assert.rejects(relayed.request({ method: 'snapshot' }));
  relayed.close();
  await assert.rejects(connectRemote({ ...options, endpoint: { mode: 'direct', url: directURL } }));
  assert.notEqual(native.device, device.publicKey);
  console.log('PASS: Node remote management, durable request association/restart recovery, image inspect/delete and log export; read-only image deletion denied; WebCrypto/WASM direct+relay, shared owner, revocation, no root key export');
} finally {
  await Promise.all(children.map(child => new Promise(resolve => {
    if (child.exitCode !== null) return resolve();
    child.once('exit', resolve); child.kill('SIGTERM');
  })));
  await rm(root, { recursive: true, force: true });
}
