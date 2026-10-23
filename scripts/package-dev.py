#!/usr/bin/env python3
"""Build and validate a local development bundle before replacing dist. No install or release signing."""
import plistlib
import shutil
import struct
import subprocess
import tempfile
from pathlib import Path

root = Path(__file__).resolve().parent.parent
subprocess.run(['cargo', 'build', '-p', 'containerhub', '--locked'], cwd=root, check=True)
local = root / '.local'
local.mkdir(exist_ok=True)
destination = root / 'dist/ContainerHub.app'
destination.parent.mkdir(exist_ok=True)
with tempfile.TemporaryDirectory(prefix='package-', dir=local) as temporary:
    staging = Path(temporary)
    bundle = staging / 'ContainerHub.app'
    contents = bundle / 'Contents'
    (contents / 'MacOS').mkdir(parents=True)
    (contents / 'Resources').mkdir()
    shutil.copy2(root / 'target/debug/containerhub', contents / 'MacOS/ContainerHub')
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
        'CFBundleIdentifier': 'dev.containerhub.desktop',
        'CFBundleName': 'ContainerHub', 'CFBundleDisplayName': 'ContainerHub',
        'CFBundleExecutable': 'ContainerHub', 'CFBundlePackageType': 'APPL',
        'CFBundleShortVersionString': '0.1.0', 'CFBundleVersion': '1',
        'CFBundleIconFile': 'AppIcon', 'LSMinimumSystemVersion': '26.0',
        'NSHighResolutionCapable': True, 'NSPrincipalClass': 'NSApplication',
    }))
    subprocess.run(['codesign', '--force', '--sign', '-', str(bundle)], check=True)
    subprocess.run(['codesign', '--verify', '--deep', '--strict', str(bundle)], check=True)
    previous = staging / 'previous.app'
    if destination.exists():
        destination.replace(previous)
    try:
        bundle.replace(destination)
    except OSError:
        if previous.exists():
            previous.replace(destination)
        raise
print(destination)
