#!/usr/bin/env python3
"""Prepare signed fork release assets. Never publish or print the private key."""
from pathlib import Path
import hashlib, os, shutil
from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

root = Path(__file__).resolve().parents[1]
version = (root / 'fork-version.txt').read_text(encoding='ascii').strip()
if not version.startswith('v') or len(version.split('.')) != 4 or not all(x.isdecimal() for x in version[1:].split('.')):
    raise SystemExit('Invalid fork version')
key_file = Path(os.environ.get('WINRUST_FORK_SIGNING_KEY', r'E:\WinRust-tools\fork-update-secret\signing-key.pem'))
key = serialization.load_pem_private_key(key_file.read_bytes(), password=None)
if not isinstance(key, Ed25519PrivateKey): raise SystemExit('Signing key is not Ed25519')
public = key.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw).hex()
if public != (root / 'fork-update-public-key.txt').read_text(encoding='ascii').strip():
    raise SystemExit('Signing key does not match the installer public key')
source = root / 'dist' / 'WinRust'
out = root / 'dist' / 'signed-release'
out.mkdir(parents=True, exist_ok=True)
assets = [('WinRust.exe', source / 'WinRust.exe')] + [(f'{code}.wrlang', source / 'locales' / f'{code}.wrlang') for code in ('pt-BR', 'ja', 'ru')]
lines = ['winrust-fork-manifest 1', f'version {version}']
for name, path in assets:
    data = path.read_bytes()
    if not data: raise SystemExit(f'Empty asset: {name}')
    lines.append(f'{hashlib.sha256(data).hexdigest()} {name}')
    (out / name).write_bytes(data)
manifest = ('\n'.join(lines) + '\n').encode('utf8')
(out / 'manifest.txt').write_bytes(manifest)
(out / 'manifest.sig').write_text(key.sign(manifest).hex() + '\n', encoding='ascii')
shutil.copy2(root / 'dist' / 'WinRust-with-languages.zip', out / 'WinRust-with-languages.zip')
print(f'Signed assets prepared in {out} for {version}')
