"""Shared build boundary for packages: explicit features and no fixture payloads."""
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def build_binary(name, features=()):
    command = [
        'cargo', 'build', '-p', name, '--locked', '--no-default-features',
        '--message-format=json',
    ]
    if features:
        command += ['--features', ','.join(features)]
    result = subprocess.run(command, cwd=ROOT, check=True, stdout=subprocess.PIPE, text=True)
    artifacts = [json.loads(line) for line in result.stdout.splitlines()]
    executable = next(
        Path(item['executable']) for item in reversed(artifacts)
        if item.get('reason') == 'compiler-artifact'
        and item.get('target', {}).get('name') == name
        and item.get('executable')
    )
    assert_no_sample_data(executable)
    return executable


def assert_no_sample_data(executable):
    data = executable.read_bytes()
    for marker in ('demo.local', 'dev.containerhub.demo', '[示例日志]'):
        if marker.encode() in data:
            raise ValueError(f'Refusing to package fixture data in {executable}')
