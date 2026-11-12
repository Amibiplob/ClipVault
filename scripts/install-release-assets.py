#!/usr/bin/env python3
"""Install shared release assets into an empty package staging directory."""
import pathlib
import shutil
import sys
import tomllib

root = pathlib.Path(__file__).resolve().parent.parent
stage = pathlib.Path(sys.argv[1]).resolve()
for source, destination, mode in tomllib.loads((root / "Cargo.toml").read_text())["package"]["metadata"]["deb"]["assets"]:
    output = stage / destination
    output.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(root / source, output)
    output.chmod(int(mode, 8))
