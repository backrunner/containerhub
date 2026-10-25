#!/usr/bin/env python3
"""Package the internal Node SDK with this host's ordinary Rust bridge. No install or publish."""
import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from package_support import ROOT, build_binary

root = ROOT
binary = build_binary('containerhub-bridge')
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
    shutil.copy2(binary, package / 'bin' / executable)
    rejected = subprocess.run(
        [str(package / 'bin' / executable), '--state', str(staging / 'demo-check'), '--demo'],
        input='', text=True, capture_output=True, timeout=15,
    )
    if rejected.returncode == 0 or '示例数据未包含在此构建中' not in rejected.stderr:
        raise ValueError('Packaged bridge must explicitly reject sample-data mode')
    (package / 'README.md').write_text(
        '# ContainerHub Node development package\n\n'
        'Node 22+; ESM SDK in `sdk/index.mjs`, types in `sdk/index.d.ts`.\n'
        'Pass the absolute path of `bin/containerhub-bridge` as `binary`, an absolute '
        '`stateDirectory`, and explicit `mode: "apple"`, `"docker"` or `"remote"` '
        'to `ContainerHub.connect`. This binary targets the build host only.\n\n'
        'Sample data and demo mode are not included in this package.\n'
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
