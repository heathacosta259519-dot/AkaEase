#!/usr/bin/env python3
"""Build a Linux tar package from the real frontend dist; never generates frontend assets."""
import argparse
import hashlib
import json
import platform
import shutil
import subprocess
import tarfile
import tempfile
from pathlib import Path

BACKEND = Path(__file__).resolve().parents[1]


def stage(binary, destination):
    """Install only owned runtime files, with no credentials, cache or development config."""
    files = {
        binary: "usr/bin/akanetease-desktop",
        BACKEND / "desktop/icons/icon.png": "usr/share/icons/hicolor/32x32/apps/io.akanetease.desktop.png",
        BACKEND / "packaging/io.akanetease.desktop.desktop": "usr/share/applications/io.akanetease.desktop.desktop",
    }
    readme_path = BACKEND / "packaging/README.md"
    if readme_path.is_file():
        files[readme_path] = "usr/share/doc/akanetease-desktop/README.md"

    for source, relative in files.items():
        target = destination / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        target.chmod(0o755 if relative.startswith("usr/bin/") else 0o644)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="check prerequisites without building")
    parser.add_argument("--offline", action="store_true")
    parser.add_argument("--output", type=Path, default=BACKEND / "dist")
    args = parser.parse_args()
    ready = (BACKEND.parent / "frontend/dist/index.html").is_file()
    if args.check:
        print(json.dumps({"linux": platform.system() == "Linux", "frontendDistReady": ready, "cargo": shutil.which("cargo") is not None, "output": str(args.output)}, indent=2))
        return 0 if ready and platform.system() == "Linux" and shutil.which("cargo") else 2
    if platform.system() != "Linux" or not ready:
        parser.error("Linux and an existing frontend/dist/index.html are required; ask the frontend workspace to build its assets first")
    command = ["cargo", "build", "--locked", "--release", "--manifest-path", str(BACKEND / "desktop/Cargo.toml"), "--bin", "akanetease-desktop", "--features", "custom-protocol", "--target-dir", str(BACKEND / "desktop/target")]
    if args.offline:
        command.append("--offline")
    subprocess.run(command, cwd=BACKEND, check=True)
    # Product version is owned by the desktop crate config.
    version = json.loads((BACKEND / "desktop/tauri.conf.json").read_text())["version"]
    name = f"akanetease-desktop-{version}-linux-{platform.machine()}"
    args.output.mkdir(parents=True, exist_ok=True)
    archive = args.output / f"{name}.tar.gz"
    if archive.exists():
        parser.error(f"output already exists: {archive}")
    with tempfile.TemporaryDirectory(prefix="aka-package-") as temp:
        root = Path(temp) / name
        stage(BACKEND / "desktop/target/release/akanetease-desktop", root)
        with tarfile.open(archive, "w:gz") as output:
            output.add(root, arcname=name)
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_suffix(archive.suffix + ".sha256").write_text(f"{digest}  {archive.name}\n")
    print(archive)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
