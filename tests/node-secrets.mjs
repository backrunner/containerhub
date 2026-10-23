import assert from 'node:assert/strict';
import { mkdtemp, rm, copyFile, chmod, writeFile, readFile, readdir } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { ContainerHub } from '../packages/node/index.mjs';

const root = await mkdtemp(join(tmpdir(), 'containerhub-secrets-'));
const options = { binary: resolve('target/debug/containerhub-bridge'), stateDirectory: join(root, 'state'), mode: 'apple', appleBinary: join(root, 'container-fixture') };
const canary = "  injection-canary=$HOME # '中文'=  ";
let client;
try {
  await copyFile('tests/fixtures/apple/cli_fixture.py', options.appleBinary); await chmod(options.appleBinary, 0o700);
  const stateFile = join(root, 'fixture-state.json');
  const state = { images: ['example.invalid/secret:v1'], containers: [], sequence: 0, missing_flags: { create: ['--mount'] } };
  await writeFile(stateFile, JSON.stringify(state));
  client = await ContainerHub.connect(options);
  const snapshot = await client.snapshot();
  assert.equal(snapshot.capabilities.create_secrets, true);
  assert.equal(snapshot.capabilities.create_advanced, false, 'secret injection has its own capability');
  const command = name => ({ kind: 'create_container', environment: snapshot.environment, spec: {
    name, image: state.images[0], cpus: 1, memory_mib: 256, options: { secret_environment: ['API_TOKEN', 'EMPTY', 'DYLD_INSERT_LIBRARIES'] },
  }});
  const secrets = { API_TOKEN: canary, EMPTY: '', DYLD_INSERT_LIBRARIES: 'not-a-host-library' };
  const created = await client.execute(command('secret-container'), { operationId: 901, secrets });
  assert.equal(created.state, 'Succeeded', created.message);
  const runtime = JSON.parse(await readFile(stateFile));
  assert.equal(runtime.injection_hash, createHash('sha256').update(`API_TOKEN=${canary}\nDYLD_INSERT_LIBRARIES=not-a-host-library\nEMPTY=\n`).digest('hex'));
  const details = await client.inspectContainer(created.result.value.key);
  assert.ok(details.environment_names.includes('API_TOKEN'));
  assert.ok(!JSON.stringify([created, details, await client.operations()]).includes(canary));
  await assert.rejects(client.submit(command('secret-container'), { operationId: 901, secrets: { ...secrets, API_TOKEN: 'changed' } }), { kind: 'Conflict' });
  assert.equal((await client.submit(command('secret-container'), { operationId: 901 })).id, 901, 'without values, duplicate ID only queries its original command');
  for (const invalid of [{}, { ...secrets, EXTRA: 'x' }, { ...secrets, API_TOKEN: 'x\nINJECTED=1' }, { ...secrets, API_TOKEN: '\u2028' }, { ...secrets, API_TOKEN: 'x'.repeat(16385) }]) {
    await assert.rejects(client.submit(command('invalid'), { secrets: invalid }), { kind: 'InvalidData' });
  }
  // Runtime failures can echo arbitrary secret input; ordinary errors stay static and redacted.
  runtime.reject_injection = true; await writeFile(stateFile, JSON.stringify(runtime));
  const rejected = await client.execute(command('rejected'), { secrets });
  assert.equal(rejected.state, 'Failed'); assert.ok(!JSON.stringify(rejected).includes(canary));
  let calls = (await readFile(join(root, 'arguments.jsonl'), 'utf8')).trim().split('\n').map(JSON.parse);
  const creates = calls.filter(args => args[0] === 'create' && !args.includes('--help'));
  assert.equal(creates.length, 2); assert.ok(!JSON.stringify(calls).includes(canary));
  assert.ok(creates.every(args => args[args.indexOf('--env-file') + 1] === '/dev/stdin'));
  await client.close(); client = undefined;
  const files = await readdir(options.stateDirectory, { recursive: true });
  const journal = join(options.stateDirectory, files.find(name => name.endsWith('operations.json')));
  const text = await readFile(journal, 'utf8'); assert.ok(!text.includes(canary));
  // Lost queued input must fail without invoking the CLI when the journal is reopened.
  const history = JSON.parse(text); const pending = structuredClone(created);
  pending.id = 902; pending.state = 'Queued'; pending.command = command('lost-input'); pending.result = null;
  history.push(pending); await writeFile(journal, JSON.stringify(history));
  client = await ContainerHub.connect(options);
  assert.equal((await client.operation(902)).state, 'Failed');
  assert.match((await client.operation(902)).message, /重新填写/);
  assert.equal((await client.operation(901)).state, 'Succeeded');
  runtime.reject_injection = false; runtime.missing_flags = { create: ['--env-file'] };
  await writeFile(stateFile, JSON.stringify(runtime));
  assert.equal((await client.diagnostics()).checks.find(c => c.feature === 'secret_environment').available, false);
  assert.equal((await client.execute(command('unsupported'), { secrets })).state, 'Failed');
  calls = (await readFile(join(root, 'arguments.jsonl'), 'utf8')).trim().split('\n').map(JSON.parse);
  assert.equal(calls.filter(args => args[0] === 'create' && !args.includes('--help')).length, 2);
  console.log('PASS: Node → queue → Apple CLI fixture stdin integrity, no argv/journal/DTO/error values, exact declarations, duplicate conflict, restart without secrets, independent capability guard');
} finally { if (client) await client.close(); await rm(root, { recursive: true, force: true }); }
