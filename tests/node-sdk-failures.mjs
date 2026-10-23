import assert from 'node:assert/strict';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { ContainerHub } from '../packages/node/index.mjs';

const directory = await mkdtemp(join(tmpdir(), 'containerhub-sdk-failure-'));
try {
  const binary = join(directory, 'bridge-fixture');
  const fixture = onEnd => `#!/usr/bin/env node
import { createInterface } from 'node:readline';
const lines = createInterface({ input: process.stdin });
lines.on('line', text => {
  const request = JSON.parse(text);
  if (request.request.method !== 'hello') { process.exit(7); }
  process.stdout.write(JSON.stringify({version:1,id:request.id,result:{protocol:1,transport:'stdio'}})+'\\n');
});
lines.on('close', () => { ${onEnd} });
`;
  // Explicit .mjs avoids Node depending on the parent directory's module type.
  const module = binary + '.mjs';
  await writeFile(module, fixture('process.exit(7);'), { mode: 0o700 });
  const options = { binary: module, stateDirectory: directory, mode: 'demo' };
  let hub = await ContainerHub.connect(options);
  await assert.rejects(hub.close(), { kind: 'UnknownOutcome' });
  hub = await ContainerHub.connect(options);
  await assert.rejects(hub.submit({ kind: 'pull_image', environment: 'demo', reference: 'example.invalid/failure:v1' }, { operationId: 123 }), { kind: 'UnknownOutcome', operationId: 123 });
  await assert.rejects(hub.close(), { kind: 'UnknownOutcome' });
  await writeFile(module, fixture('setInterval(() => {}, 1000);'), { mode: 0o700 });
  hub = await ContainerHub.connect(options);
  await assert.rejects(hub.close({ timeoutMs: 25 }), { kind: 'UnknownOutcome' });
  console.log('PASS: abnormal EOF and forced close never report success; interrupted submission preserves operation ID');
} finally { await rm(directory, { recursive: true, force: true }); }
