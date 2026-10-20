#!/usr/bin/env python3
"""Collect the license files shipped in the resolved native Rust crates."""
import json
from pathlib import Path
import subprocess
import sys


def collect(output):
    host = next(
        line.removeprefix("host: ")
        for line in subprocess.check_output(["rustc", "-vV"], text=True).splitlines()
        if line.startswith("host: ")
    )
    metadata = json.loads(subprocess.check_output([
        "cargo", "metadata", "--locked", "--format-version", "1",
        "--filter-platform", host,
    ], text=True))
    resolved = {node["id"] for node in metadata["resolve"]["nodes"]}
    sections = ["Rust dependency license notices, generated from Cargo.lock and the upstream crate archives.\n"]
    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        if package["id"] not in resolved or package["source"] is None:
            continue
        root = Path(package["manifest_path"]).parent
        files = set()
        for path in root.iterdir():
            if path.is_file() and path.name.upper().startswith(("LICENSE", "COPYING", "NOTICE")):
                files.add(path)
            elif path.is_dir() and path.name.lower() in ("license", "licenses"):
                files.update(p for p in path.rglob("*") if p.is_file())
        if package.get("license_file"):
            files.add(root / package["license_file"])
        sections.append(
            f"\n{'=' * 72}\n{package['name']} {package['version']}\n"
            f"License: {package.get('license') or 'See license file'}\n"
            f"Source: {package.get('repository') or package['source']}\n"
        )
        for path in sorted(files):
            sections.append(f"\n--- {path.relative_to(root)} ---\n")
            sections.append(path.read_text(encoding="utf-8", errors="replace"))
    Path(output).write_text("\n".join(sections), encoding="utf-8")


if __name__ == "__main__":
    collect(sys.argv[1])
