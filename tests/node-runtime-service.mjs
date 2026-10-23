import assert from 'node:assert/strict';
import { mkdtemp, rm, copyFile, chmod, writeFile, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { ContainerHub } from '../packages/node/index.mjs';

const root = await mkdtemp(join(tmpdir(), 'containerhub-runtime-service-'));
const options = { binary: resolve('target/debug/containerhub-bridge'), stateDirectory: join(root, 'state'), mode: 'apple', appleBinary: join(root, 'container-fixture') };
const stateFile = join(root, 'fixture-state.json');
const journalFile = join(options.stateDirectory, 'local', 'operations.json');
let client;
const update = async change => { const state = JSON.parse(await readFile(stateFile)); change(state); await writeFile(stateFile, JSON.stringify(state)); };
const starts = async () => (await readFile(join(root, 'arguments.jsonl'), 'utf8')).trim().split('\n').map(JSON.parse).filter(a => a[0] === 'system' && a[1] === 'start' && !a.includes('--help'));
try {
  await copyFile('tests/fixtures/apple/cli_fixture.py', options.appleBinary);
  await chmod(options.appleBinary, 0o700);
  await writeFile(stateFile, JSON.stringify({ images: [], containers: [], sequence: 0, service_offline: true }));
  client = await ContainerHub.connect(options);
  const report = await client.diagnostics();
  assert.ok(report.service_error);
  assert.ok(report.checks.find(c => c.feature === 'start_runtime').available);
  await assert.rejects(client.snapshot());
  const command = { kind: 'start_runtime', environment: report.environment };
  const first = await client.execute(command, { operationId: 100 });
  assert.equal(first.state, 'Succeeded', first.message);
  assert.equal((await client.diagnostics()).service_error, null);
  assert.equal((await client.execute(command, { operationId: 100 })).state, 'Succeeded');
  assert.equal((await client.execute(command, { operationId: 101 })).state, 'Succeeded');
  assert.deepEqual(await starts(), [['system', 'start', '--disable-kernel-install', '--timeout', '30']]);
  assert.deepEqual((await client.snapshot()).containers, []);

  await update(s => { s.service_offline = true; s.service_start_unconfirmed = true; });
  const uncertain = await client.execute(command, { operationId: 200 });
  assert.equal(uncertain.state, 'UnknownOutcome');
  await client.close(); client = await ContainerHub.connect(options);
  assert.equal((await client.operation(200)).state, 'UnknownOutcome');
  assert.equal((await starts()).length, 2);
  await client.reconcile(200);
  assert.equal((await client.wait(200)).state, 'UnknownOutcome');
  assert.equal((await starts()).length, 2, 'reconciliation never repeats startup');
  await update(s => { s.service_offline = false; });
  await client.reconcile(200);
  assert.equal((await client.wait(200)).state, 'Succeeded');
  await client.close(); client = undefined;

  // Simulate a crash checkpoint using the actual durable journal format.
  const history = JSON.parse(await readFile(journalFile));
  const interrupted = history.find(op => op.id === 200);
  interrupted.state = 'Running'; interrupted.finished_at = null; interrupted.result = null;
  await writeFile(journalFile, JSON.stringify(history));
  client = await ContainerHub.connect(options);
  assert.equal((await client.operation(200)).state, 'Succeeded');
  assert.equal((await starts()).length, 2, 'startup recovery observes only');
  await client.close(); client = undefined;
  await update(s => { s.service_offline = true; });
  await writeFile(journalFile, JSON.stringify(history));
  client = await ContainerHub.connect(options);
  assert.equal((await client.operation(200)).state, 'UnknownOutcome');
  assert.equal((await starts()).length, 2, 'offline crash recovery must not restart');
  console.log('PASS: Node runtime startup while offline, observed success, deduplication, durable uncertainty, explicit reconciliation and online/offline crash recovery without replay');
} finally { if (client) await client.close(); await rm(root, { recursive: true, force: true }); }
