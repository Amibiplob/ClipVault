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
    args = parser.parse_args()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    work = ROOT / "target/source-release"
    source = work / f"clipvault-{version}"
    archive = work / f"clipvault-{version}.tar.xz"
    archive.unlink(missing_ok=True)
    shutil.rmtree(source, ignore_errors=True)
    source.mkdir(parents=True)
    for name in ["Cargo.toml", "Cargo.lock", "src", "assets", "docs", "scripts", "packaging"]:
        path = ROOT / name
        if path.is_dir():
            shutil.copytree(path, source / name, ignore=shutil.ignore_patterns("__pycache__", "*.pyc", ".env", ".env.*"))
        else:
            shutil.copy2(path, source / name)
    for path in sorted(ROOT.iterdir()):
        if path.is_file() and (path.suffix in {".md", ".txt"} or path.name == "LICENSE"):
            shutil.copy2(path, source / path.name)
    config = subprocess.check_output(
        ["cargo", "vendor", "--locked", "--versioned-dirs", *([] if args.online else ["--offline"]), "vendor"],
        cwd=source, text=True,
    )
    (source / ".cargo").mkdir()
    (source / ".cargo/config.toml").write_text(config + '\n[net]\noffline = true\n')
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
