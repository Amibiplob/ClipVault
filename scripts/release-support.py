#!/usr/bin/env python3
"""Local release preflight. No API access, credentials, signing or publication."""
import argparse
import json
import pathlib
import re
import subprocess
import tomllib
import xml.etree.ElementTree as ET

ROOT = pathlib.Path(__file__).resolve().parents[1]
PATTERN = r'v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*))?'


def release_notes(version):
    text = (ROOT / 'CHANGELOG.md').read_text()
    match = re.search(r'^#{1,2} ' + re.escape(version) + r'(?:\s+[^\n]*)?\n(.*?)(?=^#{1,2} |\Z)', text, re.M | re.S)
    if not match or not match[1].strip():
        raise ValueError(f'Add a nonempty {version} section to CHANGELOG.md')
    return match[1].strip() + '\n'


def check(tag, commit=None):
    if not re.fullmatch(PATTERN, tag):
        raise ValueError('Expected vMAJOR.MINOR.PATCH or vMAJOR.MINOR.PATCH-prerelease')
    version = tag[1:]
    for part in (version.split('-', 1)[1].split('.') if '-' in version else []):
        if part.isdigit() and len(part) > 1 and part.startswith('0'):
            raise ValueError('Numeric prerelease identifiers must not have leading zeros')
    manifest = tomllib.loads((ROOT / 'Cargo.toml').read_text())
    lock = tomllib.loads((ROOT / 'Cargo.lock').read_text())
    locked = [p['version'] for p in lock['package'] if p['name'] == 'clipvault']
    appstream = ET.parse(ROOT / 'packaging/linux/io.clipvault.desktop.metainfo.xml').find('releases/release').get('version')
    if manifest['package']['version'] != version or locked != [version] or appstream != version:
        raise ValueError(f'{tag} must match Cargo.toml, Cargo.lock and the first AppStream release')
    if commit:
        tagged = subprocess.check_output(['git', 'rev-parse', f'refs/tags/{tag}^{{commit}}'], cwd=ROOT, text=True).strip()
        head = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
        if head != tagged or head != commit:
            raise ValueError('HEAD must be the exact resolved tagged commit')
    release_notes(version)
    return version


def required_names(version):
    return [f'clipvault_{version}-1_amd64.deb', f'clipvault-{version}-1.x86_64.rpm', f'ClipVault-{version}-x86_64.AppImage']


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--commit')
    parser.add_argument('--dist', type=pathlib.Path)
    parser.add_argument('--notes', type=pathlib.Path)
    args = parser.parse_args()
    version = check(args.tag, args.commit)
    if args.dist:
        for name in required_names(version):
            path = args.dist / name
            if not path.is_file() or path.stat().st_size < 64:
                raise ValueError(f'Missing required finished package: {name}')
    if args.notes:
        args.notes.write_text(release_notes(version))
    print(json.dumps({'version': version, 'tag': args.tag, 'prerelease': '-' in version}))
