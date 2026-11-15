#!/usr/bin/env python3
"""Offline regressions for runtime resolution and artifact ZIP inputs."""
import contextlib
import importlib.util
import io
import os
import pathlib
import subprocess
import tempfile
import unittest
from unittest.mock import patch

ROOT = pathlib.Path(__file__).resolve().parents[1]


def load(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


packaging = load("packaging-support")
upload = load("check-release-upload")


class PackagingTests(unittest.TestCase):
    def test_missing_libraries_are_aggregated_and_actionable(self):
        with patch.object(packaging, "library_paths", return_value={}):
            with self.assertRaises(RuntimeError) as error:
                packaging.runtime_inputs()
        message = str(error.exception)
        for name in ["libxcb-shape.so.0", "libsqlite3.so.0", "libwayland-egl.so.1", "libxcb-shape0", "build-packages.txt", "ldconfig -p"]:
            self.assertIn(name, message)

    def test_stage_preflight_preserves_previous_appdir(self):
        with tempfile.TemporaryDirectory() as work:
            appdir = pathlib.Path(work) / "AppDir"
            appdir.mkdir()
            previous = appdir / "keep"
            previous.write_text("previous staging")
            with patch.object(packaging, "APPDIR", appdir), patch.object(packaging, "runtime_inputs", side_effect=RuntimeError("missing runtime")):
                with self.assertRaises(RuntimeError):
                    packaging.stage()
            self.assertEqual(previous.read_text(), "previous staging")

    def test_ldconfig_skips_stale_and_wrong_architecture_entries(self):
        with tempfile.TemporaryDirectory() as work:
            good = pathlib.Path(work) / "library.so"
            good.write_bytes(b"\x7fELF\x02\x01" + bytes(12) + b"\x3e\x00")
            wrong = pathlib.Path(work) / "wrong.so"
            wrong.write_bytes(b"\x7fELF\x01\x01" + bytes(12) + b"\x03\x00")
            lines = [f"libxcb-shape.so.0 (libc6,x86-64) => {path}" for path in [good.parent / "missing", wrong, good]]
            lines.insert(0, f"libxcb-shape.so.0 (libc6) => {wrong}")
            with patch.object(packaging, "run", return_value="\n".join(lines)):
                self.assertEqual(packaging.library_paths(), {"libxcb-shape.so.0": good})

    def test_missing_transitive_dependencies_are_reported(self):
        result = subprocess.CompletedProcess([], 0, "libfixture.so.0 => not found\n", "")
        with patch.object(packaging, "library_paths", return_value={"libsqlite3.so.0": pathlib.Path("/fixture")}), patch.object(packaging, "usable_library", return_value=True), patch.object(packaging.subprocess, "run", return_value=result):
            with self.assertRaisesRegex(RuntimeError, "unresolved dependencies: libfixture.so.0"):
                packaging.runtime_inputs()


class UploadTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = pathlib.Path(self.temp.name)
        (self.root / "dist").mkdir()
        (self.root / "Cargo.toml").write_text('[package]\nversion="2.2.1"\n')
        self.package = self.root / "dist/clipvault-2.2.1-1.x86_64.rpm"
        self.package.write_bytes(b"fixture package bytes; not a real RPM")
        self.log = self.root / "dist/rpm-2.2.1-container-fixture.log"
        self.log.write_text("build log fixture")

    def run_upload(self):
        with patch.object(upload, "ROOT", self.root), patch.dict(os.environ, {"RUNNER_TEMP": str(self.root), "GITHUB_OUTPUT": str(self.root / "outputs")}), contextlib.redirect_stdout(io.StringIO()):
            return upload.main()

    def test_zip_roundtrip_and_exact_allowlist(self):
        # A private file, historical package and build tree must never be read.
        (self.root / "dist/private.pem").write_text("fixture only")
        (self.root / "dist/clipvault-2.2.0-1.x86_64.rpm").write_text("old")
        (self.root / "dist/rpm-2.2.0-container-old.log").write_text("old log")
        seen = []
        original = upload.check_zip

        def check(files, temporary):
            seen.extend(files)
            original(files, temporary)

        with patch.object(upload, "check_zip", side_effect=check):
            self.assertEqual(self.run_upload(), 0)
        self.assertEqual(seen, [self.package, self.log])
        output = (self.root / "outputs").read_text()
        self.assertIn("package=dist/clipvault-2.2.1-1.x86_64.rpm\n", output)
        diagnostics = self.root / output.splitlines()[0].split("=", 1)[1]
        self.assertEqual([path.name for path in diagnostics.iterdir()], [self.log.name])

    def test_symlink_is_rejected(self):
        self.package.unlink()
        self.package.symlink_to(self.log)
        self.assertEqual(self.run_upload(), 1)
        self.assertIn("Symlink", (self.root / "target/release-logs/fedora-upload.log").read_text())

    def test_missing_current_package_does_not_accept_historical_output(self):
        self.package.rename(self.root / "dist/clipvault-2.2.0-1.x86_64.rpm")
        self.assertEqual(self.run_upload(), 1)
        self.assertNotIn("package=", (self.root / "outputs").read_text())

    def test_rejected_log_symlink_is_not_uploaded_as_diagnostics(self):
        self.log.unlink()
        self.log.symlink_to(self.package)
        self.assertEqual(self.run_upload(), 1)
        output = (self.root / "outputs").read_text()
        diagnostics = self.root / output.splitlines()[0].split("=", 1)[1]
        self.assertEqual(list(diagnostics.iterdir()), [])

    def test_empty_package_rejected(self):
        self.package.write_bytes(b"")
        self.assertEqual(self.run_upload(), 1)

    def test_permission_error_keeps_diagnostic_report(self):
        with patch.object(upload, "check_file", side_effect=PermissionError("fixture: permission denied")):
            self.assertEqual(self.run_upload(), 1)
        self.assertIn("permission denied", (self.root / "target/release-logs/fedora-upload.log").read_text())

    def test_disk_space_failure_is_actionable(self):
        usage = type("DiskUsage", (), {"free": 0, "total": 100})()
        with patch.object(upload.shutil, "disk_usage", return_value=usage):
            self.assertEqual(self.run_upload(), 1)
        self.assertIn("Insufficient ZIP scratch space", (self.root / "target/release-logs/fedora-upload.log").read_text())

    def test_zip_io_error_is_reported(self):
        with patch.object(upload, "check_zip", side_effect=OSError("fixture: no space left")):
            self.assertEqual(self.run_upload(), 1)
        self.assertIn("no space left", (self.root / "target/release-logs/fedora-upload.log").read_text())


if __name__ == "__main__":
    unittest.main(verbosity=2)
