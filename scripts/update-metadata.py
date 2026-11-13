#!/usr/bin/env python3
"""Generate metadata from actual package bytes; optionally sign exact JSON bytes with Ed25519.
Signing keys MUST be outside this checkout. No network or publication occurs here.
"""
import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import tomllib

ROOT = pathlib.Path(__file__).resolve().parent.parent
BASE = 'https://github.com/Amibiplob/ClipVault/releases/download/'


def run(*args):
    return subprocess.check_output(args, text=True).strip()


def generate(dist, tag=None):
    version = tomllib.loads((ROOT / 'Cargo.toml').read_text())['package']['version']
    tag = tag or 'v' + version
    if not re.fullmatch(r'v?\.?\d+\.\d+\.\d+', tag) or re.sub(r'^v\.?', '', tag) != version:
        raise ValueError('Tag must match the current stable version')
    artifacts = []
    for path in sorted(dist.iterdir()):
        if not path.is_file():
            continue
        name = path.name
        platforms = []
        if name == f'clipvault_{version}-1_amd64.deb':
            identity = [run('dpkg-deb', '-f', str(path), f) for f in ('Package', 'Version', 'Architecture')]
            if identity != ['clipvault', version + '-1', 'amd64']:
                raise ValueError(f'Wrong DEB identity: {identity}')
            kind, arch = 'deb', 'x86_64'
            platforms = [{'id': 'ubuntu', 'release': '24.04'}] + [{'id': 'linuxmint', 'release': v} for v in ['22', '22.1', '22.2', '22.3']]
        elif name == f'clipvault-{version}-1.x86_64.rpm':
            identity = run('rpm', '-qp', '--qf', '%{NAME}\n%{VERSION}-%{RELEASE}\n%{ARCH}\n%{DISTRIBUTION}', str(path)).splitlines()
            if identity != ['clipvault', version + '-1', 'x86_64', 'Fedora 44']:
                raise ValueError(f'Wrong RPM identity or untested distribution: {identity}')
            kind, arch = 'rpm', 'x86_64'
            platforms = [{'id': 'fedora', 'release': '44'}]
        elif name == f'ClipVault-{version}-x86_64.AppImage':
            with path.open('rb') as f:
                header = f.read(64)
            if header[:4] != b'\x7fELF' or header[8:11] != b'AI\x02' or header[18:20] != b'\x3e\x00':
                raise ValueError('Wrong AppImage runtime/architecture')
            kind, arch = 'appimage', 'x86_64'
        else:
            continue  # Never relabel old artifacts or include logs/source bundles.
        with path.open('rb') as artifact_file:
            sha256 = hashlib.file_digest(artifact_file, 'sha256').hexdigest()
        artifacts.append({'filename': name, 'url': BASE + tag + '/' + name,
                          'package': kind, 'arch': arch, 'platforms': platforms,
                          'sha256': sha256,
                          'size': path.stat().st_size})
    if not artifacts:
        raise ValueError('No actual current-version artifacts found')
    manifest = {'schema': 1, 'repository': 'Amibiplob/ClipVault', 'version': version,
                'release_notes': (ROOT / f'CHANGELOG-v{version}.txt').read_text(), 'artifacts': artifacts}
    target = dist / 'clipvault-update.json'
    content = (json.dumps(manifest, indent=2, ensure_ascii=False) + '\n').encode()
    target.write_bytes(content)
    # Regeneration invalidates any old detached signature.
    target.with_suffix('.json.sig').unlink(missing_ok=True)
    (dist / 'v2.2.0-SHA256SUMS').write_text(''.join(f"{a['sha256']}  {a['filename']}\n" for a in artifacts))
    return target


def sign(target, key):
    key = key.resolve(strict=True)
    if key.is_relative_to(ROOT):
        raise ValueError('Signing secrets must be stored outside the repository')
    # Validate Ed25519 SPKI and require the corresponding key to be embedded in this build.
    public = subprocess.check_output(['openssl', 'pkey', '-in', str(key), '-pubout', '-outform', 'DER'])
    if public[:12].hex() != '302a300506032b6570032100' or len(public) != 44:
        raise ValueError('An Ed25519 signing key is required')
    trusted = json.loads((ROOT / 'packaging/update-keys.json').read_text())
    if public[12:].hex() not in trusted:
        raise ValueError('Public key is not embedded in packaging/update-keys.json; provision it and rebuild before signing')
    sig = target.with_suffix('.json.sig')
    subprocess.run(['openssl', 'pkeyutl', '-sign', '-rawin', '-inkey', str(key), '-in', str(target), '-out', str(sig)], check=True)
    if sig.stat().st_size != 64:
        sig.unlink()
        raise ValueError('Unexpected Ed25519 signature length')
    print('Signed exact JSON bytes with the embedded trusted Ed25519 key')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dist', type=pathlib.Path, default=ROOT / 'dist')
    parser.add_argument('--tag')
    parser.add_argument('--signing-key', type=pathlib.Path)
    opts = parser.parse_args()
    target = generate(opts.dist.resolve(), opts.tag)
    if opts.signing_key:
        sign(target, opts.signing_key)
    else:
        print('UNSIGNED metadata: automatic installation remains blocked. Provide an external Ed25519 key and rebuild with its public key.')
    print(target)


if __name__ == '__main__':
    main()
