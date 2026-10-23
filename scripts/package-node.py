#!/usr/bin/env python3
"""Package the internal Node SDK with this host's ordinary Rust bridge. No install or publish."""
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

root = Path(__file__).resolve().parent.parent
subprocess.run(['cargo', 'build', '-p', 'containerhub-bridge', '--locked'], cwd=root, check=True)
local = root / '.local'
local.mkdir(exist_ok=True)
destination = root / 'dist/node'
destination.parent.mkdir(exist_ok=True)
with tempfile.TemporaryDirectory(prefix='node-package-', dir=local) as temporary:
    staging = Path(temporary)
    package = staging / 'node'
    (package / 'bin').mkdir(parents=True)
    (package / 'sdk').mkdir()
    for name in ['index.mjs', 'index.d.ts', 'package.json']:
        shutil.copy2(root / 'packages/node' / name, package / 'sdk' / name)
    executable = 'containerhub-bridge.exe' if os.name == 'nt' else 'containerhub-bridge'
    shutil.copy2(root / 'target/debug' / executable, package / 'bin' / executable)
    (package / 'README.md').write_text(
        '# ContainerHub Node development package\n\n'
        'Node 22+; ESM SDK in `sdk/index.mjs`, types in `sdk/index.d.ts`.\n'
        'Pass the absolute path of `bin/containerhub-bridge` as `binary`, an absolute '
        '`stateDirectory`, and explicit `mode: "demo"`, `"apple"`, `"docker"` or `"remote"` '
        'to `ContainerHub.connect`. This binary targets the build host only.\n\n'
        'No npm publish, installer, production enrollment service, or release signing is included.\n'
        'Repository usage and recovery guide: `packages/node/README.md`.\n', encoding='utf-8')
    previous = staging / 'previous'
    if destination.exists():
        destination.replace(previous)
    try:
        package.replace(destination)
    except OSError:
        if previous.exists():
            previous.replace(destination)
        raise
print(destination)
