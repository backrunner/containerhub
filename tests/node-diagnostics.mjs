import assert from 'node:assert/strict';
import { mkdtemp, rm, copyFile, chmod, writeFile, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { ContainerHub } from '../packages/node/index.mjs';

const root = await mkdtemp(join(tmpdir(), 'containerhub-diagnostics-'));
const options = { binary: resolve('target/debug/containerhub-bridge'), stateDirectory: join(root, 'state'), mode: 'apple', appleBinary: join(root, 'container-fixture') };
let client;
try {
  await assert.rejects(ContainerHub.connect({ ...options, appleBinary: 'container' }), TypeError);
  await assert.rejects(ContainerHub.connect({ ...options, mode: 'demo' }), TypeError);
  await copyFile('tests/fixtures/apple/cli_fixture.py', options.appleBinary);
  await chmod(options.appleBinary, 0o700);
  const state = { images: [], containers: [], sequence: 0, service_offline: true, missing_flags: { create: ['--mount'] } };
  const stateFile = join(root, 'fixture-state.json');
  await writeFile(stateFile, JSON.stringify(state));
  client = await ContainerHub.connect(options);
  let report = await client.diagnostics();
  assert.equal(report.cli_path, options.appleBinary);
  assert.equal(report.cli_version, '1.6.0');
  assert.ok(report.service_error);
  assert.ok(report.checks.find(c => c.feature === 'create_container').available);
  assert.equal(report.checks.find(c => c.feature === 'advanced_create').available, false);
  await assert.rejects(client.snapshot());
  const host = await client.hostSnapshot();
  assert.equal(host.environment, report.environment);
  assert.equal(host.sample, false);
  assert.ok(host.memory_total_bytes > 0);
  assert.ok(!('processes' in host));
  state.service_offline = false;
  await writeFile(stateFile, JSON.stringify(state));
  report = await client.diagnostics();
  assert.equal(report.service_error, null);
  const snapshot = await client.snapshot();
  assert.equal(snapshot.capabilities.create_advanced, false);
  const command = { kind: 'create_container', environment: snapshot.environment, spec: {
    name: 'blocked', image: 'example.invalid/test:v1', cpus: 1, memory_mib: 256, ports: [], options: { read_only: true },
  }};
  assert.equal((await client.execute(command)).state, 'Failed');
  const calls = (await readFile(join(root, 'arguments.jsonl'), 'utf8')).trim().split('\n').map(JSON.parse);
  assert.ok(!calls.some(args => args[0] === 'create' && !args.includes('--help')));
  await client.close(); client = undefined;
  client = await ContainerHub.connect({ ...options, appleBinary: join(root, 'missing') });
  report = await client.diagnostics();
  assert.equal(report.cli_version, null);
  assert.equal(report.service_error.kind, 'NotInstalled');
  assert.ok(report.checks.every(c => !c.available));
  console.log('PASS: Node custom CLI, offline diagnostics, per-feature capability guards, path validation and explicit missing-path isolation');
} finally { if (client) await client.close(); await rm(root, { recursive: true, force: true }); }
