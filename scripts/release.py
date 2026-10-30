"""Builds Panda's release downloads.

    python scripts/release.py --windows --linux [--out dist]
    python scripts/release.py --host           # this machine's system (e.g. macOS)

--windows   on Windows: x86_64-pc-windows-msvc, packed as a .zip
--linux     x86_64 Linux for glibc 2.31+ (Ubuntu 20.04+, Debian 11+), packed as
            a .tar.gz: natively on Linux (needs clang), or from Windows with
            cargo-zigbuild and zig (rustup target add x86_64-unknown-linux-gnu;
            cargo install cargo-zigbuild; winget install zig.zig)
--host      whatever this machine builds by default (macOS: run it on a Mac)

Each archive holds the binary, the README files, the licences and
panda.env.example. dist/ gets `panda-<version>-<system>-<arch>.<ext>`, a
copy without the version (the "latest" link) and SHA256SUMS. The first
build downloads LiveKit's prebuilt libwebrtc (several hundred MB) and takes
a while; later builds are incremental.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import os
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXTRAS = ["README.md", "README.fr.md", "LICENSE-MIT", "LICENSE-APACHE", "NOTICE", "panda.env.example"]
LINUX = "x86_64-unknown-linux-gnu"
WINDOWS = "x86_64-pc-windows-msvc"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--windows", action="store_true")
    parser.add_argument("--linux", action="store_true")
    parser.add_argument("--host", action="store_true")
    parser.add_argument("--out", type=Path, default=ROOT / "dist")
    args = parser.parse_args()
    if not (args.windows or args.linux or args.host):
        parser.error("choose at least one of --windows, --linux, --host")
    version = re.search(r'^version\s*=\s*"([^"]+)"', (ROOT / "panda" / "Cargo.toml").read_text(encoding="utf-8"), re.M).group(1)
    args.out.mkdir(parents=True, exist_ok=True)
    made = []
    if args.windows:
        if os.name != "nt":
            sys.exit("--windows builds on Windows only")
        made.append(pack(build(WINDOWS), version, "windows-x86_64", "zip"))
    if args.linux:
        made.append(pack(build_linux(), version, "linux-x86_64", "tar.gz"))
    if args.host:
        system = {"Darwin": "macos", "Linux": "linux", "Windows": "windows"}.get(platform.system(), platform.system().lower())
        arch = {"amd64": "x86_64", "arm64": "aarch64"}.get(platform.machine().lower(), platform.machine().lower())
        made.append(pack(build(None), version, f"{system}-{arch}", "zip" if os.name == "nt" else "tar.gz"))
    sums = []
    for path in made:
        target = args.out / path.name
        shutil.move(str(path), target)
        latest = args.out / re.sub(rf"^panda-{re.escape(version)}-", "panda-", path.name)
        shutil.copyfile(target, latest)
        for file in (target, latest):
            sums.append(f"{hashlib.sha256(file.read_bytes()).hexdigest()}  {file.name}")
    sums_file = args.out / "SHA256SUMS"
    old = [line for line in sums_file.read_text().splitlines() if line.split("  ")[-1] not in {s.split("  ")[-1] for s in sums}] if sums_file.exists() else []
    sums_file.write_text("\n".join(sorted(old + sums)) + "\n", newline="\n")
    for line in sums:
        print(line)
    return 0


def cargo_env() -> dict[str, str]:
    env = dict(os.environ)
    # LiveKit's prebuilt libwebrtc for the target, never a local custom one.
    env.pop("LK_CUSTOM_WEBRTC", None)
    return env


def build(target: str | None) -> Path:
    command = ["cargo", "build", "--release", "--locked", "-p", "panda"]
    if target:
        command += ["--target", target]
    run(command, cargo_env())
    folder = target_dir() / (target or "") / "release"
    return folder / ("panda.exe" if (target or "").endswith("windows-msvc") or (not target and os.name == "nt") else "panda")


def build_linux() -> Path:
    if platform.system() == "Linux":
        return build(LINUX)
    if os.name != "nt":
        sys.exit("--linux builds on Linux, or on Windows with cargo-zigbuild")
    env = cargo_env()
    if not shutil.which("zig", path=env.get("PATH")):
        packages = Path(env.get("LOCALAPPDATA", "")) / "Microsoft" / "WinGet" / "Packages"
        found = sorted(packages.glob("zig.zig_*/zig-x86_64-windows-*/zig.exe"))
        if not found:
            sys.exit("zig not found (winget install zig.zig)")
        env["PATH"] = env["PATH"] + os.pathsep + str(found[-1].parent)
    # webrtc-sys asks pkg-config for glib's include paths, which the libwebrtc
    # sysroot already has: a stub that succeeds (and knows no libva, so the
    # VA-API encoder is left out; a bot encodes no video).
    stub = target_dir() / "pkg-config-stub.cmd"
    stub.parent.mkdir(parents=True, exist_ok=True)
    stub.write_text(
        "@echo off\r\n"
        'echo %* | findstr /C:"--variable" >nul && exit /b 1\r\n'
        'echo %* | findstr /C:"--modversion" >nul && (echo 2.66.8& exit /b 0)\r\n'
        "exit /b 0\r\n",
        encoding="ascii",
    )
    env["PKG_CONFIG"] = str(stub)
    env["PKG_CONFIG_ALLOW_CROSS"] = "1"
    run(["cargo", "zigbuild", "--release", "--locked", "-p", "panda", "--target", f"{LINUX}.2.31"], env)
    return target_dir() / LINUX / "release" / "panda"


def target_dir() -> Path:
    return Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))


def pack(binary: Path, version: str, name: str, kind: str) -> Path:
    if not binary.is_file():
        sys.exit(f"{binary} was not built")
    folder = f"panda-{version}-{name}"
    archive = target_dir() / f"{folder}.{kind}"
    files = [(binary, binary.name)] + [(ROOT / extra, extra) for extra in EXTRAS]
    if kind == "zip":
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as out:
            for path, inside in files:
                out.write(path, f"{folder}/{inside}")
    else:
        with tarfile.open(archive, "w:gz") as out:
            for path, inside in files:
                info = tarfile.TarInfo(f"{folder}/{inside}")
                data = path.read_bytes()
                info.size = len(data)
                info.mode = 0o755 if path == binary else 0o644
                info.mtime = int(path.stat().st_mtime)
                out.addfile(info, io.BytesIO(data))
    print(f"packed {archive.name} ({archive.stat().st_size // (1024 * 1024)} MiB)")
    return archive


def run(command: list[str], env: dict[str, str]) -> None:
    print("+ " + " ".join(command))
    if subprocess.run(command, cwd=ROOT, env=env).returncode != 0:
        sys.exit(f"{command[0]} {command[1]} failed")


if __name__ == "__main__":
    sys.exit(main())
