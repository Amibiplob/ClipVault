#!/usr/bin/env python3
"""Create a deterministic source archive with complete Cargo vendoring.

An explicit --online preparation may fetch missing crates. Builders remain offline.
Git, build outputs, environment files, private data and credentials are excluded.
"""
import argparse
import os
import pathlib
import shutil
import subprocess
import tarfile
import tomllib

ROOT = pathlib.Path(__file__).resolve().parent.parent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--online", action="store_true")
    parser.add_argument("--linux-ppa", action="store_true",
                        help="Repack unused Windows fixtures/libraries for the Linux PPA")
    parser.add_argument("--output-dir", type=pathlib.Path,
                        help="Separate staging directory (preserves prior source archives)")
    args = parser.parse_args()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    work = (args.output_dir or ROOT / "target/source-release").resolve()
    if work == ROOT or ROOT.is_relative_to(work):
        parser.error("Output must not be the source root or one of its parents")
    source = work / f"clipvault-{version}"
    archive = work / f"clipvault-{version}.tar.xz"
    archive.unlink(missing_ok=True)
    shutil.rmtree(source, ignore_errors=True)
    source.mkdir(parents=True)
    for name in ["Cargo.toml", "Cargo.lock", "src", "assets", "docs", "scripts", "packaging"]:
        path = ROOT / name
        if path.is_dir():
            shutil.copytree(path, source / name, ignore=shutil.ignore_patterns(
                "__pycache__", "*.pyc", ".env", ".env.*", ".gnupg", "private-keys-v1.d",
                "*.key", "*.pem", "*.p12", "*.pfx", "*.gpg", "*.kbx", "*.asc"))
        else:
            shutil.copy2(path, source / name)
    for path in sorted(ROOT.iterdir()):
        if path.is_file() and (path.suffix in {".md", ".txt"} or path.name == "LICENSE"):
            shutil.copy2(path, source / path.name)
    for path in source.rglob("*"):
        if path.is_file() and (b"-----BEGIN PGP " + b"PRIVATE KEY BLOCK-----") in path.read_bytes():
            raise RuntimeError(f"Private signing key found in source input: {path.relative_to(source)}")
    config = subprocess.check_output(
        ["cargo", "vendor", "--locked", "--versioned-dirs", *([] if args.online else ["--offline"]), "vendor"],
        cwd=source, text=True,
    )
    (source / ".cargo").mkdir()
    (source / ".cargo/config.toml").write_text(config + '\n[net]\noffline = true\n')
    if args.linux_ppa:
        subprocess.run(["python3", str(ROOT / "scripts/repack-ppa-vendor.py"), str(source)],
                       check=True)
    cargo_home = work / "empty-cargo-home"
    shutil.rmtree(cargo_home, ignore_errors=True)
    cargo_home.mkdir()
    subprocess.run(["cargo", "metadata", "--locked", "--offline", "--format-version", "1"],
                   cwd=source, env={**os.environ, "CARGO_HOME": str(cargo_home)},
                   check=True, stdout=subprocess.DEVNULL)
    # Include all upstream license/notice/checksum files, including other targets.
    archive = work / f"clipvault-{version}.tar.xz"
    epoch = int(os.environ.get("SOURCE_DATE_EPOCH", "1794484800"))

    def normalize(info):
        info.uid = info.gid = 0
        info.uname = info.gname = "root"
        info.mtime = epoch
        return info

    with tarfile.open(archive, "w:xz", format=tarfile.PAX_FORMAT) as output:
        output.add(source, arcname=source.name, filter=normalize)
    print(archive)


if __name__ == "__main__":
    main()
