#!/usr/bin/env python3
"""Isolated Docker protocol fixture. Never contacts an Engine or invokes Docker."""
import fcntl
import json
import pathlib
import sys

root = pathlib.Path(__file__).parent
lock_file = (root / 'fixture.lock').open('a')
fcntl.flock(lock_file, fcntl.LOCK_EX)
state_file = root / 'state.json'
state = json.loads(state_file.read_text())
def save_state():
    temporary = state_file.with_suffix('.next')
    temporary.write_text(json.dumps(state))
    temporary.replace(state_file)
args = sys.argv[1:]
with (root / 'arguments.jsonl').open('a') as file:
    file.write(json.dumps(args) + '\n')
assert args[:2] == ['--context', 'fixture']
args = args[2:]
if state.get('offline'):
    print('private diagnostic token-do-not-leak', file=sys.stderr)
    sys.exit(1)
if args[0] == 'info':
    print(json.dumps({'ID': state.get('engine', 'fixture-engine'), 'ServerVersion': 'fixture-1', 'Swarm': state.get('swarm', {})}))
elif args[:2] == ['service', 'ls']:
    if state.get('swarm_fail'): sys.exit(1)
    selector = args[args.index('--filter') + 1]
    services = state.get('services', [])
    if selector.startswith('id='): services = [s for s in services if s['ID'].startswith(selector[3:])]
    print('\n'.join(s['ID'][:12] for s in services))
elif args[:2] == ['service', 'inspect']:
    services = [s for s in state.get('services', []) if s['ID'].startswith(args[-1])]
    if not services: sys.exit(1)
    print(json.dumps(services))
elif args[:2] == ['service', 'ps']:
    print('\n'.join(t['ID'] for t in state.get('tasks', []) if t['ServiceID'] == args[-1]))
elif args[:3] == ['inspect', '--type', 'task']:
    print(json.dumps([t for t in state.get('tasks', []) if t['ID'] in args[4:]]))
elif args[:2] == ['service', 'update']:
    assert len(args[-1]) == 25
    service = next(s for s in state['services'] if s['ID'] == args[-1])
    assert args[-2] == '--'
    if '--replicas' in args:
        service['Spec']['Mode']['Replicated']['Replicas'] = int(args[args.index('--replicas') + 1])
    elif '--force' in args:
        service['Spec']['TaskTemplate']['ForceUpdate'] += 1
    else: raise AssertionError(args)
    service['Version']['Index'] += 1
    save_state()
    print(service['ID'])
elif args[:2] == ['service', 'rm']:
    assert len(args[-1]) == 25
    if not state.get('ignore_remove'): state['services'] = [s for s in state['services'] if s['ID'] != args[-1]]
    save_state()
    print(args[-1])
elif args[:2] == ['container', 'ls']:
    print('\n'.join(c['Id'] for c in state['containers']))
elif args[:2] == ['container', 'inspect']:
    ids = args[3:]
    found = [c for c in state['containers'] if c['Id'] in ids]
    if len(found) != len(ids):
        sys.exit(1)
    print(json.dumps(found))
elif args[:2] == ['container', 'create']:
    name = args[args.index('--name') + 1]
    if any(c['Name'] == '/' + name for c in state['containers']):
        sys.exit(1)
    seq = state.get('sequence', 100) + 1
    state['sequence'] = seq
    c = {'Id': format(seq, '064x'), 'Name': '/' + name, 'Platform': 'linux',
         'Config': {'Image': args[-1], 'Labels': {}, 'Env': []},
         'HostConfig': {'AutoRemove': False}, 'State': {'Status': 'created'},
         'NetworkSettings': {'Networks': {}, 'Ports': {}}}
    state['containers'].append(c)
    save_state()
    print(c['Id'])
elif args[0] == 'container' and args[1] in ['start', 'stop', 'restart', 'rm']:
    assert args[2] == '--'
    id_ = args[3]
    found = next((c for c in state['containers'] if c['Id'] == id_), None)
    if found is None:
        sys.exit(1)
    if args[1] == 'rm':
        assert found['State']['Status'] in ['created', 'exited']
        state['containers'].remove(found)
    else:
        found['State']['Status'] = 'exited' if args[1] == 'stop' else 'running'
    save_state()
    print(id_)
elif args[0] in ['image', 'volume', 'network'] and args[1] == 'ls':
    pass
else:
    raise AssertionError(args)
