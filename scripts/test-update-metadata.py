#!/usr/bin/env python3
"""Isolated metadata/signing-tool tests. Temporary keys are test fixtures, not release keys."""
import importlib.util
import json
import pathlib
import subprocess
import tempfile
import unittest

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
        (metadata.ROOT / 'CHANGELOG-v2.2.0.txt').write_text('Test release')
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

    def test_signing_requires_external_key_embedded_public_key_and_verifiable_signature(self):
        key = self.base / 'test-only.pem'
        subprocess.run(['openssl', 'genpkey', '-algorithm', 'ED25519', '-out', str(key)], check=True)
        target = metadata.generate(self.dist)
        with self.assertRaises(ValueError):
            metadata.sign(target, key)
        public = subprocess.check_output(['openssl', 'pkey', '-in', str(key), '-pubout', '-outform', 'DER'])
        (metadata.ROOT / 'packaging/update-keys.json').write_text(json.dumps([public[12:].hex()]))
        metadata.sign(target, key)
        public_file = self.base / 'public.der'
        public_file.write_bytes(public)
        verify = ['openssl', 'pkeyutl', '-verify', '-pubin', '-keyform', 'DER', '-inkey', str(public_file), '-rawin', '-in', str(target), '-sigfile', str(target.with_suffix('.json.sig'))]
        subprocess.run(verify, check=True)
        target.write_bytes(target.read_bytes() + b' ')
        self.assertNotEqual(subprocess.run(verify, stdout=subprocess.DEVNULL).returncode, 0)
        inside = metadata.ROOT / 'forbidden.pem'
        inside.write_bytes(key.read_bytes())
        with self.assertRaises(ValueError):
            metadata.sign(target, inside)


if __name__ == '__main__':
    unittest.main()
