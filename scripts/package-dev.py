#!/usr/bin/env python3
"""Build, validate and install ContainerHub (Dev). No release signing or fixture data."""
import argparse
import hashlib
import json
import plistlib
import shutil
import struct
import subprocess
import tempfile
from pathlib import Path
from package_support import ROOT, build_binary

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--no-install', action='store_true', help='Write only dist/ContainerHub (Dev).app')
args = parser.parse_args()
root = ROOT
executable = build_binary('containerhub', features=['dev-build'])
info = json.loads(subprocess.check_output([str(executable), '--build-info'], text=True))
expected = {
    'name': 'ContainerHub (Dev)',
    'app_id': 'dev.containerhub.desktop.dev',
    'channel': 'dev',
    'data_directory': 'ContainerHub (Dev)',
    'sample_data': False,
    'ui_validation': False,
    'remote_development': False,
}
if any(info.get(key) != value for key, value in expected.items()):
    raise ValueError(f'Refusing to package an unexpected build: {info}')
local = root / '.local'
local.mkdir(exist_ok=True)
bundle_name = info['name'] + '.app'
destination = (root / 'dist' if args.no_install else Path('/Applications')) / bundle_name
destination.parent.mkdir(exist_ok=True)
# Replacing a running executable leaves the old process alive. Require a clean restart.
processes = subprocess.check_output(['ps', '-axo', 'comm='], text=True).splitlines()
if str(destination / 'Contents/MacOS/ContainerHub') in (line.strip() for line in processes):
    raise RuntimeError(f'Quit {destination.name} before updating it')
with tempfile.TemporaryDirectory(prefix='package-', dir=local) as temporary:
    staging = Path(temporary)
    bundle = staging / bundle_name
    contents = bundle / 'Contents'
    (contents / 'MacOS').mkdir(parents=True)
    (contents / 'Resources').mkdir()
    shutil.copy2(executable, contents / 'MacOS/ContainerHub')
    # ICNS stores PNG representations in typed, big-endian length-prefixed blocks.
    # Keep conversion independent of iconutil's system-service availability.
    # Format mapping: https://github.com/python-pillow/Pillow/blob/main/src/PIL/IcnsImagePlugin.py
    representations = {
        b'icp4': 16, b'icp5': 32, b'icp6': 64, b'ic07': 128,
        b'ic08': 256, b'ic09': 512, b'ic10': 1024,
        b'ic11': 32, b'ic12': 64, b'ic13': 256, b'ic14': 512,
    }
    pngs = {}
    for pixels in sorted(set(representations.values())):
        image = staging / f'{pixels}.png'
        subprocess.run(['sips', '-z', str(pixels), str(pixels), str(root / 'assets/app-icon.png'), '--out', str(image)], stdout=subprocess.DEVNULL, check=True)
        data = image.read_bytes()
        if data[:8] != b'\x89PNG\r\n\x1a\n' or struct.unpack('>II', data[16:24]) != (pixels, pixels):
            raise ValueError(f'Invalid {pixels}px icon representation')
        pngs[pixels] = data
    blocks = b''.join(struct.pack('>4sI', kind, len(pngs[pixels]) + 8) + pngs[pixels] for kind, pixels in representations.items())
    icon = contents / 'Resources/AppIcon.icns'
    icon.write_bytes(struct.pack('>4sI', b'icns', len(blocks) + 8) + blocks)
    subprocess.run(['sips', '-g', 'pixelWidth', '-g', 'pixelHeight', str(icon)], check=True)
    (contents / 'Info.plist').write_bytes(plistlib.dumps({
        'CFBundleIdentifier': info['app_id'],
        'CFBundleName': info['name'], 'CFBundleDisplayName': info['name'],
        'CFBundleExecutable': 'ContainerHub', 'CFBundlePackageType': 'APPL',
        'CFBundleShortVersionString': info['version'], 'CFBundleVersion': '1',
        'ContainerHubBuildChannel': info['channel'],
        'CFBundleIconFile': 'AppIcon', 'LSMinimumSystemVersion': '26.0',
        'NSHighResolutionCapable': True, 'NSPrincipalClass': 'NSApplication',
    }))
    subprocess.run(['codesign', '--force', '--sign', '-', str(bundle)], check=True)
    subprocess.run(['codesign', '--verify', '--deep', '--strict', str(bundle)], check=True)
    # Stage beside the destination so installation also works across volumes.
    with tempfile.TemporaryDirectory(prefix='.containerhub-dev-', dir=destination.parent) as install:
        ready = Path(install) / bundle_name
        shutil.copytree(bundle, ready)
        subprocess.run(['codesign', '--verify', '--deep', '--strict', str(ready)], check=True)
        previous = Path(install) / 'previous.app'
        if destination.exists():
            destination.replace(previous)
        try:
            ready.replace(destination)
        except OSError:
            if previous.exists():
                previous.replace(destination)
            raise
    installed = destination / 'Contents/MacOS/ContainerHub'
    if hashlib.sha256(installed.read_bytes()).digest() != hashlib.sha256((contents / 'MacOS/ContainerHub').read_bytes()).digest():
        raise ValueError('Installed executable differs from the signed development bundle')
print(destination)
