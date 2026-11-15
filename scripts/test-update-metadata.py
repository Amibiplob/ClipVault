#!/usr/bin/env python3
"""Isolated metadata/signing-tool tests. Temporary keys are test fixtures, not release keys."""
import importlib.util
import json
import pathlib
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('metadata', pathlib.Path(__file__).with_name('update-metadata.py'))
metadata = importlib.util.module_from_spec(spec)
spec.loader.exec_module(metadata)


class MetadataTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='cv-signing-test-')
        self.base = pathlib.Path(self.temp.name)
        self.previous = metadata.ROOT
        metadata.ROOT = self.base / 'repo'
        metadata.ROOT.mkdir()
        (metadata.ROOT / 'Cargo.toml').write_text('[package]\nversion="2.2.0"\n')
        (metadata.ROOT / 'CHANGELOG.md').write_text('## 2.2.0 — Test\nTest release\n')
        (metadata.ROOT / 'packaging').mkdir()
        (metadata.ROOT / 'packaging/update-keys.json').write_text('[]')
        self.dist = metadata.ROOT / 'dist'
        self.dist.mkdir()
        header = bytearray(64)
        header[:4] = b'\x7fELF'
        header[8:11] = b'AI\x02'
        header[18:20] = b'\x3e\x00'
        (self.dist / 'ClipVault-2.2.0-x86_64.AppImage').write_bytes(header)

    def tearDown(self):
        metadata.ROOT = self.previous
        self.temp.cleanup()

    def test_actual_artifact_sizes_checksums_and_obsolete_signature_removed(self):
        (self.dist / 'ClipVault-2.1.0-x86_64.AppImage').write_bytes(b'old untouched')
        (self.dist / 'clipvault-update.json.sig').write_bytes(b'obsolete')
        manifest = json.loads(metadata.generate(self.dist).read_text())
        self.assertEqual(len(manifest['artifacts']), 1)
        self.assertEqual(manifest['artifacts'][0]['size'], 64)
        self.assertFalse((self.dist / 'clipvault-update.json.sig').exists())
        self.assertEqual((self.dist / 'ClipVault-2.1.0-x86_64.AppImage').read_bytes(), b'old untouched')
        subprocess.run(['sha256sum', '--check', 'v2.2.0-SHA256SUMS'], cwd=self.dist, check=True)
        with self.assertRaises(ValueError):
            metadata.generate(self.dist, 'v2.3.0')

    def test_single_job_never_inspects_unrelated_old_rpm(self):
        (self.dist / 'clipvault-2.2.0-1.x86_64.rpm').write_bytes(b'previous unrelated candidate')
        manifest = json.loads(metadata.generate(self.dist, packages=['appimage']).read_text())
        self.assertEqual([a['package'] for a in manifest['artifacts']], ['appimage'])

    def test_prerelease_deb_version_and_urls(self):
        version = '2.2.1-beta.1'
        (metadata.ROOT / 'Cargo.toml').write_text(f'[package]\nversion="{version}"\n')
        (metadata.ROOT / 'CHANGELOG.md').write_text(f'## {version} — Test\nPreview notes\n')
        package = self.base / 'fixture-deb'
        (package / 'DEBIAN').mkdir(parents=True)
        (package / 'DEBIAN/control').write_text('Package: clipvault\nVersion: 2.2.1~beta.1-1\nArchitecture: amd64\nMaintainer: Fixture <fixture@example.invalid>\nDescription: Metadata test only\n')
        deb = self.dist / f'clipvault_{version}-1_amd64.deb'
        subprocess.run(['dpkg-deb', '--build', '--root-owner-group', str(package), str(deb)], check=True, stdout=subprocess.DEVNULL)
        manifest = json.loads(metadata.generate(self.dist, 'v' + version).read_text())
        self.assertEqual(manifest['version'], version)
        self.assertEqual(manifest['artifacts'][0]['url'], metadata.BASE + 'v' + version + '/' + deb.name)
        self.assertTrue((self.dist / f'v{version}-SHA256SUMS').exists())
        subprocess.run(['dpkg', '--compare-versions', '2.2.1~beta.1-1', 'lt', '2.2.1-1'], check=True)

    def test_fedora_rpm_identity_and_platform_rules(self):
        rpm = self.dist / 'clipvault-2.2.0-1.x86_64.rpm'
        rpm.write_bytes(b'mock RPM bytes')
        with patch.object(metadata, 'run', return_value='clipvault\n2.2.0-1\nx86_64\nFedora 44'):
            manifest = json.loads(metadata.generate(self.dist, packages=['rpm']).read_text())
        self.assertEqual(manifest['artifacts'][0]['platforms'], [{'id':'fedora','release':'44'}])
        with patch.object(metadata, 'run', return_value='clipvault\n2.2.0-1\nx86_64\nFedora 45'):
            with self.assertRaises(ValueError):
                metadata.generate(self.dist, packages=['rpm'])

    def test_signing_requires_external_key_embedded_public_key_and_verifiable_signature(self):
        key = self.base / 'test-only.pem'
        subprocess.run(['openssl', 'genpkey', '-algorithm', 'ED25519', '-out', str(key)], check=True)
        target = metadata.generate(self.dist)
        with self.assertRaises(ValueError):
            metadata.sign(target, key)
        public = subprocess.check_output(['openssl', 'pkey', '-in', str(key), '-pubout', '-outform', 'DER'])
        (metadata.ROOT / 'packaging/update-keys.json').write_text(json.dumps([public[12:].hex()]))
        metadata.sign(target, key)
        metadata.verify(target)
        public_file = self.base / 'public.der'
        public_file.write_bytes(public)
        verify = ['openssl', 'pkeyutl', '-verify', '-pubin', '-keyform', 'DER', '-inkey', str(public_file), '-rawin', '-in', str(target), '-sigfile', str(target.with_suffix('.json.sig'))]
        subprocess.run(verify, check=True)
        target.write_bytes(target.read_bytes() + b' ')
        with self.assertRaises(ValueError):
            metadata.verify(target)
        self.assertNotEqual(subprocess.run(verify, stdout=subprocess.DEVNULL).returncode, 0)
        inside = metadata.ROOT / 'forbidden.pem'
        inside.write_bytes(key.read_bytes())
        with self.assertRaises(ValueError):
            metadata.sign(target, inside)


if __name__ == '__main__':
    unittest.main()
