#!/usr/bin/env python3
"""Check only the current RPM and its build logs before Actions ZIP upload.

This checks transport inputs, not RPM compilation/installation or signing.
"""
import os
import pathlib
import shutil
import stat
import tempfile
import tomllib
import zipfile

ROOT = pathlib.Path(__file__).resolve().parent.parent


def check_file(path):
    # Never follow a symlink into a build tree, user data, or signing material.
    if path.parent.is_symlink() or path.is_symlink():
        raise RuntimeError(f"Symlink is not an upload input: {path}")
    info = path.stat()
    if not stat.S_ISREG(info.st_mode) or info.st_size == 0:
        raise RuntimeError(f"Expected a nonempty regular upload file: {path}")
    # Opening matters: ownership alone does not establish readability.
    with path.open("rb") as source:
        source.read(1)
    return info


def check_zip(files, temporary):
    # Exercise an actual disk-backed ZIP and read its CRCs before Actions runs.
    # Do not archive the source/build tree or environment/private key files.
    required = sum(check_file(path).st_size for path in files) + 64 * 1024 * 1024
    if shutil.disk_usage(temporary).free < required:
        raise RuntimeError(f"Insufficient ZIP scratch space at {temporary}: need at least {required} bytes. "
                           "Free generated build/cache space or use a larger runner; keep package validation enabled.")
    with tempfile.TemporaryDirectory(prefix="clipvault-upload-", dir=temporary) as work:
        archive = pathlib.Path(work) / "upload-check.zip"
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, allowZip64=True) as output:
            for path in files:
                output.write(path, path.name)
        with zipfile.ZipFile(archive) as output:
            bad = output.testzip()
            if bad:
                raise RuntimeError(f"Upload ZIP CRC check failed: {bad}")


def main():
    report = ROOT / "target/release-logs/fedora-upload.log"
    report.parent.mkdir(parents=True, exist_ok=True)
    lines = []

    def record(message):
        lines.append(message)
        print(message, flush=True)
        report.write_text("\n".join(lines) + "\n")

    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    package = ROOT / f"dist/clipvault-{version}-1.x86_64.rpm"
    logs = sorted((ROOT / "dist").glob(f"rpm-{version}-*.log"))
    diagnostics = pathlib.Path(tempfile.mkdtemp(prefix="fedora-", dir=report.parent))
    if output := os.environ.get("GITHUB_OUTPUT"):
        with open(output, "a") as dest:
            dest.write(f"diagnostics={diagnostics.relative_to(ROOT)}\n")
    temporary = pathlib.Path(os.environ.get("RUNNER_TEMP", tempfile.gettempdir()))
    record("RPM upload input diagnostics (does not establish RPM build/validation success)")
    record("Full hosted upload-step logs must still be reviewed to establish the original ZIP failure cause.")
    errors = []
    for directory in (ROOT / "dist", temporary):
        try:
            usage = shutil.disk_usage(directory)
            record(f"Disk {directory}: free={usage.free} total={usage.total}")
        except OSError as error:
            errors.append(f"Cannot inspect disk {directory}: {error}")
    files = [package, *logs]
    for path in files:
        try:
            info = check_file(path)
            record(f"Input {path.relative_to(ROOT)}: bytes={info.st_size} mode={stat.filemode(info.st_mode)} uid={info.st_uid} gid={info.st_gid}; readable regular file")
            if path in logs:
                # Snapshot only inputs that passed inspection. An always() upload
                # must never follow a rejected log symlink or unreadable file.
                snapshot = diagnostics / path.name
                shutil.copyfile(path, snapshot)
                snapshot.chmod(0o644)
        except (OSError, RuntimeError) as error:
            errors.append(f"{path.relative_to(ROOT)}: {error}. Check container output permissions; never upload target/rpmbuild or the whole checkout.")
    if not logs:
        errors.append("No current-version RPM build logs found; inspect the build step before retrying upload.")
    if not errors:
        try:
            check_zip(files, temporary)
            record(f"PASS: ZIP creation and CRC readback for exactly {len(files)} current-version files")
        except (OSError, RuntimeError, zipfile.BadZipFile) as error:
            errors.append(f"ZIP preflight failed: {error}")
    if errors:
        for error in errors:
            record("ERROR: " + error)
        return 1
    if output:
        with open(output, "a") as dest:
            dest.write(f"package={package.relative_to(ROOT)}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
