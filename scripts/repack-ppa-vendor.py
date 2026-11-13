#!/usr/bin/env python3
"""Repack exactly the unused Windows files documented in PPA-SOURCE-REPACK.md.

Only called for fresh Linux PPA sources, after complete cargo vendor. Never edit
the registry cache, Cargo.lock, crate metadata, licenses or remaining file hashes.
"""
import argparse
import hashlib
import json
import pathlib
import tomllib


POLICY = {
    "libloading-0.8.9": (
        "d7c4b02199fee7c5d21a5ae7d8cfa79a6ef5bb2fc834d6e9058e89c825efdc55",
        ["tests/nagisa32.dll", "tests/nagisa64.dll"],
        2,
    ),
    "winapi-i686-pc-windows-gnu-0.4.0": (
        "ac3b87c63620426dd9b991e5ce0329eff545bccbbb34f3be09ff6fb6ab51b7b6",
        ["lib/*.a"],
        1387,
    ),
    "winapi-x86_64-pc-windows-gnu-0.4.0": (
        "712e227841d057c1ee1cd2fb22fa7e5a5461ae8e48fa2ca79ec42cfc1931183f",
        ["lib/*.a"],
        1416,
    ),
}


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify(source):
    """Check every retained vendor byte and registry package against Cargo.lock."""
    lock = tomllib.loads((source / "Cargo.lock").read_text())
    packages = {f"{p['name']}-{p['version']}": p["checksum"]
                for p in lock["package"] if p.get("source", "").startswith("registry+")}
    vendor = source / "vendor"
    assert {p.name for p in vendor.iterdir()} == set(packages), "Vendor/lock package mismatch"
    for name, checksum in sorted(packages.items()):
        crate = vendor / name
        manifest = json.loads((crate / ".cargo-checksum.json").read_text())
        assert manifest["package"] == checksum, f"Package checksum changed: {name}"
        actual = {p.relative_to(crate).as_posix() for p in crate.rglob("*")
                  if p.is_file() and p != crate / ".cargo-checksum.json"}
        assert actual == set(manifest["files"]), f"File inventory mismatch: {name}"
        for relative, expected in manifest["files"].items():
            assert digest(crate / relative) == expected, f"File checksum mismatch: {name}/{relative}"
    return len(packages)


def repack(source):
    count = verify(source)  # Reject incomplete, modified or unexpected input first.
    plans = []
    for name, (checksum, patterns, expected_count) in POLICY.items():
        crate = source / "vendor" / name
        path = crate / ".cargo-checksum.json"
        manifest = json.loads(path.read_text())
        assert manifest["package"] == checksum, f"Unreviewed upstream release: {name}"
        paths = sorted({p for pattern in patterns for p in crate.glob(pattern)})
        assert len(paths) == expected_count, f"Unreviewed file inventory: {name}"
        removed = {p.relative_to(crate).as_posix(): manifest["files"][p.relative_to(crate).as_posix()]
                   for p in paths}
        plans.append((crate, path, manifest, paths, {
            "crate": name, "package": checksum,
            "original_checksum_manifest_sha256": digest(path), "removed": removed,
        }))
    report = {"policy": "linux-ppa-windows-files-v1", "cargo_lock_sha256": digest(source / "Cargo.lock"),
              "registry_packages": count, "crates": []}
    for crate, path, manifest, paths, record in plans:
        for removed in paths:
            del manifest["files"][removed.relative_to(crate).as_posix()]
            removed.unlink()
        # Keep the registry .crate checksum and every remaining per-file hash.
        path.write_text(json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n")
        record["repacked_checksum_manifest_sha256"] = digest(path)
        report["crates"].append(record)
    verify(source)
    (source / "packaging/PPA-VENDOR-REPACK.json").write_text(
        json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"Verified {count} registry packages; repacked 2805 unused Windows files")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--verify", action="store_true", help="Verify retained vendor bytes without modifying them")
    parser.add_argument("source", type=pathlib.Path)
    args = parser.parse_args()
    if args.verify:
        print(f"Verified {verify(args.source.resolve())} registry packages")
    else:
        repack(args.source.resolve())


if __name__ == "__main__":
    main()
