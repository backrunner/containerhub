#!/usr/bin/env python3
"""Isolated public-CLI contract simulator. Never launches or connects to a container runtime."""
import json
from pathlib import Path
import sys
import time

root = Path(__file__).parent
state_file = root / 'fixture-state.json'
state = json.loads(state_file.read_text()) if state_file.exists() else {'images': [], 'containers': [], 'sequence': 0}
args = sys.argv[1:]
with (root / 'arguments.jsonl').open('a') as log:
    log.write(json.dumps(args) + '\n')

def emit(value):
    state_file.write_text(json.dumps(state))
    print(json.dumps(value))

def missing():
    print('not found', file=sys.stderr)
    raise SystemExit(1)

def image_exists(name):
    return name in state['images'] or ('@' in name and any(digest(n)==name.split('@')[-1] for n in state['images']))

def digest(name):
    return state.get('digests', {}).get(name, 'sha256:' + 'a' * 64)

if args == ['--version']:
    print('container CLI version ' + state.get('version', '1.6.0'))
elif args[-1:] == ['--help'] and len(args) <= 3:
    command = ' '.join(args[:-1])
    flags = {
        'list': ['--all', '--format'], 'image list': ['--format'], 'volume list': ['--format'], 'network list': ['--format'],
        'logs': ['-n', '--boot'], 'inspect': [], 'image inspect': [], 'stats': ['--format', '--no-stream'],
        'start': [], 'stop': ['--time'], 'image pull': ['--progress'], 'delete': [], 'image delete': [], 'system status': ['--format'], 'system start': ['--disable-kernel-install', '--timeout'],
        'create': ['--name', '--cpus', '--memory', '--publish', '--label', '--network', '--mount', '--user', '--workdir', '--read-only', '--env-file', '--entrypoint', '--platform', '--runtime', '--tty', '--ulimit', '--rosetta', '--virtualization', '--ssh', '--init', '--shm-size', '--cap-add', '--cap-drop', '--masked-path', '--read-only-path', '--no-dns', '--dns', '--dns-option', '--dns-search', '--dns-domain'],
    }
    if command not in flags or command in state.get('missing_commands', []): missing()
    print('USAGE: container ' + command + ' [options]')
    for flag in flags[command]:
        if flag not in state.get('missing_flags', {}).get(command, []): print('  ' + flag + ' <value>  option')
    if command == 'image pull': print('progress format: auto|none|ansi|plain|color')
elif args[:2] == ['system', 'start']:
    assert args == ['system', 'start', '--disable-kernel-install', '--timeout', '30']
    time.sleep(state.get('service_start_delay', 0))
    state['service_start_count'] = state.get('service_start_count', 0) + 1
    if state.get('service_start_denied'):
        emit(None)
        print('permission denied fixture-secret', file=sys.stderr)
        raise SystemExit(1)
    if not state.get('service_start_unconfirmed'):
        state['service_offline'] = False
    emit(None)
    if state.get('service_start_exit_error'): raise SystemExit(1)
elif args[:2] == ['system', 'status']:
    if state.get('service_status_invalid'):
        print('invalid status'); raise SystemExit(0)

    if state.get('service_offline'): missing()
    print(json.dumps({'status': 'running'}))
elif args[:1] == ['list']:
    emit(state['containers'])
elif args[:2] == ['image', 'pull']:
    print('[1/2] Fetching image 10% (fixture-secret)', file=sys.stderr, flush=True)
    time.sleep(state.get('pull_delay', 0))
    print('[2/2] Unpacking image', file=sys.stderr, flush=True)
    time.sleep(state.get('unpack_delay', 0))
    if args[-1] not in state['images']:
        state['images'].append(args[-1])
    emit(None)
elif args[:2] in (['image', 'inspect'], ['image', 'list']):
    names = state['images'] if args[1] == 'list' else [args[-1]]
    if any(not image_exists(name) for name in names):
        missing()
    emit([{'id': digest(name).split(':')[1], 'configuration': {'name': name, 'creationDate': '2026-10-22T00:00:00Z', 'descriptor': {'digest': digest(name), 'mediaType': 'application/vnd.oci.image.index.v1+json'}}, 'variants': [{'platform': {'os': 'linux', 'architecture': 'arm64'}, 'digest': 'sha256:' + 'b' * 64, 'size': 4096, 'config': {'config': {'Env': ['PATH=/bin', 'TOKEN=fixture-secret']}}}]} for name in names])
elif args[:2] == ['image', 'delete']:
    assert args == ['image', 'delete', '--', args[-1]]
    if args[-1] not in state['images']:
        missing()
    state['images'].remove(args[-1])
    emit(None)
    if state.get('fail_cleanup'):
        print('orphan cleanup failed', file=sys.stderr)
        raise SystemExit(1)
elif args[:2] in (['network', 'list'], ['volume', 'list']):
    name = 'default' if args[0] == 'network' else 'database-data'
    emit([{'id': name, 'configuration': {'name': name, 'mode': 'nat', 'driver': 'local', 'format': 'ext4'}}])
elif args[:1] == ['inspect']:
    container = next((c for c in state['containers'] if c['id'] == args[-1]), None)
    if not container: missing()
    emit([container])
elif args[:1] == ['create']:
    separator = 1
    while args[separator].startswith('--'):
        option = args[separator]
        assert option != '--'
        separator += 1 if option in ['--read-only','--no-dns','--tty','--init','--rosetta','--virtualization','--ssh'] or option.startswith('--entrypoint=') else 2
    if state.get('fail_create_after_delete') and state.get('deleted'):
        print('fixture create failed', file=sys.stderr); raise SystemExit(1)
    image = args[separator]
    overrides = args[separator + 1:]
    args = args[:separator]  # Never interpret passthrough arguments as CLI options.
    entrypoint = next((arg.split('=', 1)[1] for arg in args if arg.startswith('--entrypoint=')), None)
    image_process = state.get('image_process', {}).get(image, {'entrypoint': ['/fixture-init'], 'arguments': ['fixture-secret-argument']})
    prefix = [entrypoint] if entrypoint is not None else image_process.get('entrypoint', [])
    full_process = prefix + (overrides if overrides else ([] if entrypoint is not None else image_process.get('arguments', [])))
    assert full_process and full_process[0]
    injected = []
    if '--env-file' in args:
        assert args[args.index('--env-file') + 1] == '/dev/stdin'
        data = sys.stdin.buffer.read()
        import hashlib
        state['injection_hash'] = hashlib.sha256(data).hexdigest()
        injected = data.decode().splitlines()
        if state.get('reject_injection'):
            print(data.decode(), file=sys.stderr)
            raise SystemExit(1)
    name = args[args.index('--name') + 1]
    assert all(c['id'] != name for c in state['containers'])
    state['sequence'] += 1
    env = dict(line.split('=',1) for line in ['TOKEN=fixture-secret'] + injected)
    published=[]
    for i,a in enumerate(args):
        if a=='--publish':
            address,rest=args[i+1].rsplit(':',2)[0],args[i+1].rsplit(':',2)[1:]
            target,proto=rest[1].split('/')
            published.append({'hostAddress':address.strip('[]'),'hostPort':int(rest[0]),'containerPort':int(target),'proto':proto})
    state['containers'].append({'id': name, 'status': {'state': 'stopped'}, 'configuration': {
        'image': {'reference': image, 'descriptor': {'digest': digest(image)}},
        'creationDate': f'fixture-{state["sequence"]}',
        'resources': {'cpus': int(args[args.index('--cpus') + 1]), 'memoryInBytes': int(args[args.index('--memory') + 1][:-1]) * 1024 * 1024},
        'platform': {'os': 'linux', 'architecture': 'arm64'},
        'networks': [{'network': args[i+1]} for i, a in enumerate(args) if a == '--network'],
        'mounts': [{'type':{'volume':{'name':a.split('source=')[1].split(',')[0],'format':'ext4','cache':'on','sync':'fsync'}},'source':a.split('source=')[1].split(',')[0], 'destination': a.split('target=')[1].split(',')[0], 'options':['ro'] if a.endswith(',readonly') else []} for a in args if a.startswith('type=volume,')],
        'publishedPorts':published,
        'readOnly': '--read-only' in args,
        'labels': dict(args[i+1].split('=',1) for i,a in enumerate(args) if a=='--label'),
        'initProcess': {'environment': [k+'='+v for k,v in env.items()], 'executable': full_process[0], 'arguments': full_process[1:],
            'workingDirectory': args[args.index('--workdir') + 1] if '--workdir' in args else '/',
            'user': {'id': {'uid': 1000, 'gid': 1000}}}

    }})
    emit(name)
elif args[:1] in (['start'], ['stop'], ['delete'], ['stats'], ['logs']):
    container = next((c for c in state['containers'] if c['id'] == args[-1]), None)
    if not container:
        missing()
    if args[0] == 'delete':
        assert container['status']['state'] == 'stopped'
        state['containers'].remove(container)
        state['deleted']=state.get('deleted',0)+1
        emit(None)
    elif args[0] == 'stats':
        emit([{'id': container['id'], 'cpuUsageUsec': 1000, 'memoryUsageBytes': 123456, 'memoryLimitBytes': container['configuration']['resources']['memoryInBytes']}])
    elif args[0] == 'logs':
        print('fixture log')
    else:
        if args[0] == 'start' and state.get('fail_start'):
            print('fixture start rejected', file=sys.stderr)
            raise SystemExit(1)
        container['status']['state'] = 'running' if args[0] == 'start' else 'stopped'
        emit(None)
else:
    raise AssertionError(f'Unexpected command {args}')
