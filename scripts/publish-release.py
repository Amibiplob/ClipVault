#!/usr/bin/env python3
"""Publish a complete validated bundle. Published releases are strictly read-only on retries.

--dry-run performs local validation only; it never reads a token or contacts GitHub.
"""
import argparse
import hashlib
import importlib.util
import json
import os
import pathlib
import subprocess
import urllib.error
import urllib.parse
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts' / f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def bundle(dist, tag):
    release = load('release-support')
    version = release.check(tag)
    names = release.required_names(version) + ['clipvault-update.json', 'clipvault-update.json.sig', f'{tag}-SHA256SUMS']
    for name in names:
        if not (dist / name).is_file():
            raise ValueError(f'Missing release file: {name}')
    metadata = load('update-metadata')
    metadata.verify(dist / 'clipvault-update.json')
    manifest = json.loads((dist / 'clipvault-update.json').read_bytes())
    if manifest['schema'] != 1 or manifest['repository'] != 'Amibiplob/ClipVault' or manifest['version'] != version:
        raise ValueError('Wrong signed manifest identity')
    if sorted(a['filename'] for a in manifest['artifacts']) != sorted(names[:3]):
        raise ValueError('Manifest must contain exactly all three packages')
    for asset in manifest['artifacts']:
        path = dist / asset['filename']
        if asset['sha256'] != hashlib.sha256(path.read_bytes()).hexdigest() or asset['size'] != path.stat().st_size:
            raise ValueError('Signed manifest does not match packaged bytes')
        if asset['url'] != metadata.BASE + tag + '/' + path.name:
            raise ValueError('Wrong release download URL')
    expected = ''.join(f"{a['sha256']}  {a['filename']}\n" for a in manifest['artifacts'])
    if (dist / f'{tag}-SHA256SUMS').read_text() != expected:
        raise ValueError('Checksums disagree with signed manifest')
    return [dist / name for name in names]


class SafeRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        parsed = urllib.parse.urlsplit(newurl)
        if parsed.scheme != 'https' or parsed.hostname not in ('api.github.com', 'release-assets.githubusercontent.com') or parsed.username or parsed.password:
            raise ValueError('Unexpected release asset redirect')
        redirected = super().redirect_request(req, fp, code, msg, headers, newurl)
        if parsed.hostname != 'api.github.com':
            redirected.remove_header('Authorization')
        return redirected


class GitHub:
    def __init__(self, repository, token):
        if repository != 'Amibiplob/ClipVault':
            raise ValueError('Publication restricted to the canonical repository')
        self.root = f'https://api.github.com/repos/{repository}'
        self.token = token

    def request(self, method, path, data=None, raw=False, content_type=None):
        url = path if path.startswith('https://') else self.root + path
        headers = {'Authorization': f'Bearer {self.token}', 'User-Agent': 'clipvault-release',
                   'Accept': 'application/octet-stream' if raw else 'application/vnd.github+json',
                   'X-GitHub-Api-Version': '2022-11-28'}
        if data is not None:
            headers['Content-Type'] = content_type or 'application/json'
            if not isinstance(data, bytes):
                data = json.dumps(data).encode()
        try:
            with urllib.request.build_opener(SafeRedirect()).open(urllib.request.Request(url, data=data, headers=headers, method=method), timeout=120) as response:
                result = response.read()
                return result if raw else (json.loads(result) if result else None)
        except urllib.error.HTTPError as error:
            if error.code == 404 and method == 'GET' and path.startswith('/releases/tags/'):
                return None
            raise RuntimeError(f'GitHub {method} failed: HTTP {error.code}; draft remains unpublished') from None


def publish(api, files, tag, commit, notes):
    existing = api.request('GET', '/releases/tags/' + urllib.parse.quote(tag, safe=''))
    if existing and existing.get('target_commitish') != commit:
        raise ValueError('Existing release is not bound to this exact commit; inspect it manually')
    # Complete the comparison before making any mutation. A published retry is read-only.
    remote = {a['name']: a for a in (existing or {}).get('assets', [])}
    names = {f.name for f in files}
    if set(remote) - names:
        raise ValueError('Release contains unexpected assets; inspect manually before retrying')
    same = set()
    for file in files:
        asset = remote.get(file.name)
        if asset and api.request('GET', f"/releases/assets/{asset['id']}", raw=True) == file.read_bytes():
            same.add(file.name)
    if existing and not existing['draft']:
        if same != names:
            raise ValueError('Published release differs: refusing to replace any published binary or metadata. Use a new version/tag.')
        print('Published release matches every local byte; read-only retry completed.')
        return
    if not existing:
        existing = api.request('POST', '/releases', {'tag_name': tag, 'target_commitish': commit,
                               'name': 'ClipVault ' + tag, 'body': notes, 'draft': True, 'prerelease': '-' in tag})
    for file in files:
        if file.name in same:
            continue
        if file.name in remote:
            api.request('DELETE', f"/releases/assets/{remote[file.name]['id']}")
        upload = existing['upload_url'].split('{', 1)[0] + '?name=' + urllib.parse.quote(file.name, safe='')
        result = api.request('POST', upload, file.read_bytes(), content_type='application/octet-stream')
        if api.request('GET', f"/releases/assets/{result['id']}", raw=True) != file.read_bytes():
            raise ValueError('Uploaded bytes differ; draft remains unpublished')
    api.request('PATCH', f"/releases/{existing['id']}", {'draft': False, 'body': notes, 'prerelease': '-' in tag, 'make_latest': 'false' if '-' in tag else 'legacy'})
    print(f'Published {tag} with {len(files)} verified downloadable assets.')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--tag', required=True)
    parser.add_argument('--commit', required=True)
    parser.add_argument('--dist', type=pathlib.Path, default=ROOT / 'dist')
    parser.add_argument('--dry-run', action='store_true')
    opts = parser.parse_args()
    release = load('release-support')
    version = release.check(opts.tag, opts.commit)
    files = bundle(opts.dist, opts.tag)
    if opts.dry_run:
        print('NONPUBLISHING: complete signed bundle validated; no GitHub requests made.')
    else:
        publish(GitHub(os.environ['GITHUB_REPOSITORY'], os.environ['GH_TOKEN']), files, opts.tag,
                opts.commit, release.release_notes(version))
