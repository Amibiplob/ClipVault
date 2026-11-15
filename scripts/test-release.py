#!/usr/bin/env python3
"""Nonpublishing release regression tests. All GitHub calls use an in-memory fake."""
import importlib.util
import contextlib
import io
import pathlib
import tempfile
import unittest
from unittest.mock import patch

ROOT = pathlib.Path(__file__).resolve().parents[1]


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts' / f'{name}.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


release = load('release-support')
publisher = load('publish-release')


class FakeGitHub:
    def __init__(self, release=None, bodies=None, fail=False):
        self.release = release
        self.bodies = bodies or {}
        self.calls = []
        self.fail = fail

    def request(self, method, path, data=None, **kwargs):
        self.calls.append((method, path))
        if method == 'GET' and path.startswith('/releases/tags/'):
            return self.release
        if method == 'GET':
            return self.bodies[int(path.rsplit('/', 1)[1])]
        if method == 'POST' and path == '/releases':
            self.release = dict(data, id=1, upload_url='https://uploads.github.com/repos/Amibiplob/ClipVault/releases/1/assets{?name,label}')
            return self.release
        if method == 'POST':
            if self.fail:
                raise RuntimeError('fixture upload failure')
            key = len(self.bodies) + 1
            self.bodies[key] = data
            return {'id': key}
        if method == 'PATCH':
            self.release.update(data)
        return None


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.capture = contextlib.redirect_stdout(io.StringIO())
        self.capture.__enter__()
        self.temp = tempfile.TemporaryDirectory()
        self.file = pathlib.Path(self.temp.name) / 'package.deb'
        self.file.write_bytes(b'final bytes')

    def tearDown(self):
        self.temp.cleanup()
        self.capture.__exit__(None, None, None)

    def test_version_validation(self):
        version = __import__('tomllib').loads((ROOT / 'Cargo.toml').read_text())['package']['version']
        self.assertEqual(release.check('v' + version), version)
        for tag in ['v99.0.0', 'v2.2', 'main', 'v02.2.0', 'v2.2.0-beta.01', 'v2.2.0; echo secret']:
            with self.assertRaises(ValueError):
                release.check(tag)
        self.assertTrue(release.release_notes(version).strip())

    def test_exact_tagged_commit_is_required(self):
        version = __import__('tomllib').loads((ROOT / 'Cargo.toml').read_text())['package']['version']
        with patch.object(release.subprocess, 'check_output', side_effect=['abc\n', 'abc\n']):
            self.assertEqual(release.check('v' + version, 'abc'), version)
        with patch.object(release.subprocess, 'check_output', side_effect=['retargeted\n', 'abc\n']):
            with self.assertRaises(ValueError):
                release.check('v' + version, 'abc')

    def test_all_packages_required(self):
        self.assertEqual(len(release.required_names('2.2.1')), 3)
        with self.assertRaises(ValueError):
            publisher.bundle(pathlib.Path(self.temp.name), 'v2.2.0')

    def test_complete_new_release_uploaded_verified_then_published(self):
        api = FakeGitHub()
        publisher.publish(api, [self.file], 'v2.2.1', 'abc', 'notes')
        self.assertFalse(api.release['draft'])
        self.assertEqual(api.calls[-1], ('PATCH', '/releases/1'))
        self.assertEqual(api.bodies[1], self.file.read_bytes())

    def test_upload_failure_keeps_draft_unpublished(self):
        api = FakeGitHub(fail=True)
        with self.assertRaises(RuntimeError):
            publisher.publish(api, [self.file], 'v2.2.1', 'abc', 'notes')
        self.assertTrue(api.release['draft'])
        self.assertFalse(any(m == 'PATCH' for m, _ in api.calls))

    def test_published_different_bytes_refused_without_mutation(self):
        api = self.existing(False, b'different')
        with self.assertRaises(ValueError):
            publisher.publish(api, [self.file], 'v2.2.1', 'abc', 'notes')
        self.assertTrue(all(m == 'GET' for m, _ in api.calls))

    def existing(self, draft, content):
        return FakeGitHub({'id':1,'draft':draft,'target_commitish':'abc', 'assets':[{'id':1,'name':self.file.name}], 'upload_url':'https://uploads.github.com/repos/Amibiplob/ClipVault/releases/1/assets{?name,label}'}, {1:content})

    def test_published_identical_retry_is_read_only(self):
        api = self.existing(False, self.file.read_bytes())
        publisher.publish(api, [self.file], 'v2.2.1', 'abc', 'notes')
        self.assertTrue(all(m == 'GET' for m, _ in api.calls))

    def test_draft_identical_retry_skips_upload(self):
        api = self.existing(True, self.file.read_bytes())
        publisher.publish(api, [self.file], 'v2.2.1', 'abc', 'notes')
        self.assertEqual(len([1 for m, _ in api.calls if m == 'POST']), 0)
        self.assertFalse(api.release['draft'])

    def test_draft_changed_bytes_replaced_before_publication(self):
        api = self.existing(True, b'old candidate')
        publisher.publish(api, [self.file], 'v2.2.1-beta.1', 'abc', 'notes')
        self.assertIn(('DELETE','/releases/assets/1'), api.calls)
        self.assertTrue(api.release['prerelease'])

    def test_wrong_commit_or_extra_assets_refused(self):
        api = self.existing(True, b'old')
        with self.assertRaises(ValueError):
            publisher.publish(api, [self.file], 'v2.2.1', 'other-commit', 'notes')
        api.release['assets'].append({'id':2,'name':'unexpected'})
        with self.assertRaises(ValueError):
            publisher.publish(api, [self.file], 'v2.2.1', 'abc', 'notes')
        self.assertTrue(all(m == 'GET' for m, _ in api.calls))


if __name__ == '__main__':
    unittest.main()
