import assert from 'node:assert/strict';
import { mkdtemp, rm, copyFile, chmod, writeFile, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { ContainerHub } from '../packages/node/index.mjs';

const root = await mkdtemp(join(tmpdir(), 'containerhub-startup-'));
const options = { binary: resolve('target/debug/containerhub-bridge'), stateDirectory: join(root, 'state'), mode: 'apple', appleBinary: join(root, 'container-fixture') };
const reference = 'example.invalid/command:v1';
let client;
try {
  await copyFile('tests/fixtures/apple/cli_fixture.py', options.appleBinary); await chmod(options.appleBinary, 0o700);
  const stateFile = join(root, 'fixture-state.json');
  const state = { images: [reference], containers: [], sequence: 0, missing_flags: { create: ['--mount'] }, image_process: { [reference]: { entrypoint: ['/image-init', 'image-prefix'], arguments: ['image-default'] } } };
  await writeFile(stateFile, JSON.stringify(state));
  client = await ContainerHub.connect(options);
  const snapshot = await client.snapshot();
  assert.equal(snapshot.capabilities.create_startup, true);
  assert.equal(snapshot.capabilities.create_advanced, false);
  const command = (name, startup) => ({ kind: 'create_container', environment: snapshot.environment, spec: {
    name, image: reference, cpus: 1, memory_mib: 256, options: { startup },
  }});
  const literal = ['--name', 'untouched', '--entrypoint=/unexpected', '--', '', 'two words', "'quoted'", '$HOME; $(echo no)', '中文\n第二行', '--help'];
  const cases = [
    ['defaults', undefined, '/image-init', ['image-prefix', 'image-default']],
    ['args-only', { arguments: literal }, '/image-init', ['image-prefix', ...literal]],
    ['program-only', { entrypoint: '/path with spaces/app' }, '/path with spaces/app', []],
    ['program-args', { entrypoint: '/app', arguments: literal }, '/app', literal],
  ];
  let last;
  for (const [name, startup, executable, args] of cases) {
    const result = await client.execute(command(name, startup));
    assert.equal(result.state, 'Succeeded', result.message);
    assert.equal(result.result.value.state, 'Stopped');
    const runtime = JSON.parse(await readFile(stateFile));
    const process = runtime.containers.find(c => c.id === name).configuration.initProcess;
    assert.equal(process.executable, executable);
    assert.deepEqual(process.arguments, args);
    const details = await client.inspectContainer(result.result.value.key);
    assert.equal(details.argument_count, args.length);
    assert.ok(!JSON.stringify(details).includes('$HOME;'));
    last = result;
  }
  const [sameA, sameB] = await Promise.all([client.submit(command('dedup', { entrypoint: '/app', arguments: ['a', 'b'] }), { operationId: 500 }), client.submit(command('dedup', { entrypoint: '/app', arguments: ['a', 'b'] }), { operationId: 500 })]);
  assert.equal(sameA.id, sameB.id); assert.equal((await client.wait(500)).state, 'Succeeded');
  await assert.rejects(client.submit(command('dedup', { entrypoint: '/app', arguments: ['b', 'a'] }), { operationId: 500 }), { kind: 'Conflict' });
  for (const startup of [{}, { entrypoint: '' }, { arguments: ['x\0y'] }, { arguments: ['x'.repeat(4097)] }, { arguments: Array(65).fill('') }, { arguments: Array(8).fill('x'.repeat(4096)) }]) {
    await assert.rejects(client.submit(command('invalid', startup)), { kind: 'InvalidData' });
  }
  let calls = (await readFile(join(root, 'arguments.jsonl'), 'utf8')).trim().split('\n').map(JSON.parse);
  let creates = calls.filter(args => args[0] === 'create' && args.length > 3);
  assert.equal(creates.length, 5);
  assert.deepEqual(creates[1].slice(creates[1].indexOf(reference) + 1), literal);
  assert.ok(creates[2].includes('--entrypoint=/path with spaces/app'));
  await client.close(); client = undefined;
  client = await ContainerHub.connect(options);
  assert.deepEqual((await client.operation(last.id)).command, last.command);
  assert.equal((await client.operation(last.id)).state, 'Succeeded');
  const runtime = JSON.parse(await readFile(stateFile)); runtime.missing_flags = { create: ['--entrypoint'] };
  await writeFile(stateFile, JSON.stringify(runtime));
  assert.equal((await client.diagnostics()).checks.find(c => c.feature === 'startup_command').available, false);
  assert.equal((await client.execute(command('unsupported', { arguments: ['x'] }))).state, 'Failed');
  assert.equal((await client.execute(command('default-still-works'))).state, 'Succeeded');
  calls = (await readFile(join(root, 'arguments.jsonl'), 'utf8')).trim().split('\n').map(JSON.parse);
  creates = calls.filter(args => args[0] === 'create' && args.length > 3);
  assert.equal(creates.length, 6);
  assert.ok(creates.every(args => !args.includes('unsupported')));
  console.log('PASS: Node → bridge → Apple CLI fixture image/default/entrypoint/CMD semantics, literal argv including flags/empty/Unicode, independent capability, bounded validation, durable dedupe and restart without replay');
} finally { if (client) await client.close(); await rm(root, { recursive: true, force: true }); }
