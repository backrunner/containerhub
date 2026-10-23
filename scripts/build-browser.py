#!/usr/bin/env python3
"""Build the shared protocol for browsers and Node interop tests; no global toolchain changes."""
import argparse
import os
from pathlib import Path
import shutil
import subprocess

root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('--rustc', help='Compiler whose sysroot includes wasm32-unknown-unknown')
parser.add_argument('--wasm-bindgen', help='wasm-bindgen CLI matching Cargo.lock (0.2.129)')
parser.add_argument('--offline', action='store_true')
args = parser.parse_args()
env = os.environ.copy()
if args.rustc:
    env['RUSTC'] = str(Path(args.rustc).resolve())
bindgen = args.wasm_bindgen or shutil.which('wasm-bindgen')
if not bindgen:
    candidates = sorted((root / '.local/tools').glob('wasm-bindgen-0.2.129-*/wasm-bindgen'))
    if len(candidates) == 1:
        bindgen = str(candidates[0])
if not bindgen:
    parser.error('Install wasm-bindgen-cli 0.2.129 or pass --wasm-bindgen /path/to/wasm-bindgen')
version = subprocess.check_output([bindgen, '--version'], text=True).strip()
if version != 'wasm-bindgen 0.2.129':
    parser.error(f'Expected wasm-bindgen 0.2.129; found {version}')
command = ['cargo', 'build', '-p', 'containerhub-remote', '--no-default-features', '--features', 'browser', '--target', 'wasm32-unknown-unknown', '--locked']
if args.offline:
    command.append('--offline')
subprocess.run(command, cwd=root, env=env, check=True)
source = root / 'target/wasm32-unknown-unknown/debug/containerhub_remote.wasm'
for target, output in [('nodejs', '.local/browser-protocol'), ('web', 'dist/browser-protocol')]:
    subprocess.run([bindgen, '--target', target, '--out-dir', str(root / output), str(source)], check=True)
shutil.copy2(root / 'packages/client-js/remote.mjs', root / 'dist/browser-protocol/remote.mjs')
print('Generated .local/browser-protocol (Node tests) and dist/browser-protocol (browser modules)')
