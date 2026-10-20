#!/usr/bin/env python3
"""Native packages, run on the target OS. Only verified upstream engines are bundled."""
import bz2
import hashlib
import os
from pathlib import Path
import platform
import plistlib
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile
import urllib.request
import zipfile
import zlib
import re

ROOT = Path(__file__).resolve().parent.parent
os.chdir(ROOT)
VERSION = re.search(r'^version = "([^"]+)"', (ROOT / "Cargo.toml").read_text(), re.M)[1]
SYSTEM = platform.system()
assert SYSTEM in ("Linux", "Darwin"), "Run this script on Ubuntu or macOS"
ARM = platform.machine() in ("arm64", "aarch64")
ARCH = "arm64" if ARM else "x64"
UPSTREAM_ARCH = "arm64" if ARM else "amd64"
UPSTREAM_OS = "darwin" if SYSTEM == "Darwin" else "linux"
TOOLS = ROOT / ".tools"
TOOLS.mkdir(exist_ok=True)

def fetch(url, path):
    print(f"Downloading {url}", flush=True)
    request = urllib.request.Request(url, headers={"User-Agent": "Straja-packaging"})
    with urllib.request.urlopen(request, timeout=120) as response, open(path, "wb") as output:
        shutil.copyfileobj(response, output)

def verified(url, sums_url, filename):
    archive = TOOLS / filename
    fetch(url, archive)
    sums = TOOLS / (filename + ".checksums")
    fetch(sums_url, sums)
    lines = [line.split() for line in sums.read_text().splitlines()]
    expected = next(parts[0] for parts in lines if len(parts) == 2 and parts[1].lstrip("*") == filename)
    actual = hashlib.sha256(archive.read_bytes()).hexdigest()
    assert actual.lower() == expected.lower(), f"Checksum mismatch: {filename}"
    return archive

restic_version = "0.19.1"
name = f"restic_{restic_version}_{UPSTREAM_OS}_{UPSTREAM_ARCH}.bz2"
base = f"https://github.com/restic/restic/releases/download/v{restic_version}"
archive = verified(f"{base}/{name}", f"{base}/SHA256SUMS", name)
(TOOLS / "restic").write_bytes(bz2.decompress(archive.read_bytes()))
rclone_version = "1.75.2"
name = f"rclone-v{rclone_version}-{'osx' if SYSTEM == 'Darwin' else 'linux'}-{UPSTREAM_ARCH}.zip"
base = f"https://github.com/rclone/rclone/releases/download/v{rclone_version}"
archive = verified(f"{base}/{name}", f"{base}/SHA256SUMS", name)
with zipfile.ZipFile(archive) as z:
    member = next(n for n in z.namelist() if n.endswith("/rclone"))
    (TOOLS / "rclone").write_bytes(z.read(member))
for name in ("restic", "rclone"):
    (TOOLS / name).chmod(0o755)
    subprocess.run([str(TOOLS / name), "version"], check=True)
if "--tools-only" in sys.argv:
    sys.exit(0)

OUT = ROOT / "dist/native"
OUT.mkdir(parents=True, exist_ok=True)

def copy_payload(destination):
    destination.mkdir(parents=True, exist_ok=True)
    subprocess.run([sys.executable, str(ROOT / "scripts/rust-license-notices.py"),
                    str(destination / "RUST-DEPENDENCY-LICENSES.txt")], check=True)
    for name in ("straja", "straja-agent"):
        shutil.copy2(ROOT / "target/release" / name, destination / name)
    for name in ("restic", "rclone"):
        shutil.copy2(TOOLS / name, destination / name)
    for name in ("LICENSE", "THIRD-PARTY-NOTICES.txt", "ARCHIVE-LICENSES.txt", "GRAPHICS-LICENSES.txt", "RCLONE-LICENSE.txt", "NATIVE.md"):
        shutil.copy2(ROOT / name, destination / name)
    shutil.copy2(ROOT / "scripts/enable-autostart.sh", destination / "enable-autostart.sh")
    (destination / "enable-autostart.sh").chmod(0o755)

def png_from_rgba(path):
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    rgba = (ROOT / "installer/assets/straja.rgba").read_bytes()
    rows = b"".join(b"\0" + rgba[y*512:(y+1)*512] for y in range(128))
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB",128,128,8,6,0,0,0)) + chunk(b"IDAT",zlib.compress(rows)) + chunk(b"IEND",b""))

with tempfile.TemporaryDirectory(prefix="straja-package-") as temp:
    stage = Path(temp)
    if SYSTEM == "Linux":
        package = stage / "deb"
        payload = package / "opt/straja-backup"
        copy_payload(payload)
        desktop = package / "usr/share/applications"
        desktop.mkdir(parents=True)
        (desktop / "straja-backup.desktop").write_text(
            "[Desktop Entry]\nType=Application\nName=Straja Backup\nComment=Back up and restore your files\n"
            "Exec=/opt/straja-backup/straja\nIcon=straja-backup\nTerminal=false\nCategories=Utility;Archiving;\n")
        icons = package / "usr/share/icons/hicolor/128x128/apps"
        icons.mkdir(parents=True)
        png_from_rgba(icons / "straja-backup.png")
        control = package / "DEBIAN"
        control.mkdir()
        (control / "control").write_text(
            f"Package: straja-backup\nVersion: {VERSION}\nArchitecture: {'arm64' if ARM else 'amd64'}\n"
            "Maintainer: Weblike PRO\nSection: utils\nPriority: optional\n"
            "Depends: libc6 (>= 2.35), libgcc-s1, libdbus-1-3, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libx11-6, libxcursor1, libxi6, libxrandr2, libegl1, xdg-utils\n"
            "Recommends: gnome-keyring\nHomepage: https://strajabackup.com\n"
            "Description: Straja Backup desktop application and background agent\n"
            " Local and cloud backups with schedules, exclusions, retention and restore.\n")
        subprocess.run(["dpkg-deb", "--root-owner-group", "--build", str(package),
                        str(OUT / f"StrajaBackup-{VERSION}-ubuntu-{ARCH}.deb")], check=True)
        with tarfile.open(OUT / f"StrajaBackup-{VERSION}-linux-{ARCH}.tar.gz", "w:gz") as tar:
            tar.add(payload, arcname="StrajaBackup")
    else:
        app = stage / "image/Straja Backup.app"
        payload = app / "Contents/MacOS"
        copy_payload(payload)
        resources = app / "Contents/Resources"
        resources.mkdir(parents=True)
        # macOS reserves MacOS/ for native code. Documents and shell helpers
        # are sealed resources, not nested executable code objects.
        for path in list(payload.iterdir()):
            if path.name not in ("straja", "straja-agent", "restic", "rclone"):
                shutil.move(str(path), str(resources / path.name))
        png = stage / "icon.png"
        png_from_rgba(png)
        iconset = stage / "Straja.iconset"
        iconset.mkdir()
        for size in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                target = iconset / f"icon_{size}x{size}{'@2x' if scale == 2 else ''}.png"
                subprocess.run(["sips", "-z", str(size*scale), str(size*scale), str(png), "--out", str(target)], check=True, stdout=subprocess.DEVNULL)
        subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(resources / "Straja.icns")], check=True)
        with open(app / "Contents/Info.plist", "wb") as f:
            plistlib.dump({
                "CFBundleName": "Straja Backup", "CFBundleDisplayName": "Straja Backup",
                "CFBundleIdentifier": "com.weblike.straja-backup", "CFBundleExecutable": "straja",
                "CFBundlePackageType": "APPL", "CFBundleVersion": VERSION,
                "CFBundleShortVersionString": VERSION, "CFBundleIconFile": "Straja.icns",
                "LSMinimumSystemVersion": "14.0", "NSHighResolutionCapable": True,
            }, f)
        # Ad-hoc signatures allow native execution on Apple Silicon. This is NOT Developer ID signing.
        for name in ("restic", "rclone", "straja-agent"):
            subprocess.run(["codesign", "--force", "--sign", "-", str(payload / name)], check=True)
        subprocess.run(["codesign", "--force", "--sign", "-", str(app)], check=True)
        subprocess.run(["codesign", "--verify", "--deep", "--strict", str(app)], check=True)
        (stage / "image/Applications").symlink_to("/Applications")
        shutil.copy2(ROOT / "NATIVE.md", stage / "image/READ-ME-FIRST.txt")
        subprocess.run(["hdiutil", "create", "-volname", "Straja Backup", "-srcfolder", str(stage / "image"),
                        "-ov", "-format", "UDZO", str(OUT / f"StrajaBackup-{VERSION}-macos-{ARCH}.dmg")], check=True)
        with tarfile.open(OUT / f"StrajaBackup-{VERSION}-macos-{ARCH}.tar.gz", "w:gz") as tar:
            tar.add(app, arcname=app.name)
for path in OUT.iterdir():
    if path.suffix in (".deb", ".dmg", ".gz"):
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        path.with_name(path.name + ".sha256").write_text(f"{digest}  {path.name}\n")
print(f"Native kits: {OUT}")
