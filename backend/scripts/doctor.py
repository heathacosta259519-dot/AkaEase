#!/usr/bin/env python3
"""Read-only Linux runtime/build checks; never opens the keyring or contacts NetEase."""
import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path


def run(command):
    try:
        return subprocess.run(command, capture_output=True, text=True, timeout=15).returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False


def checks(build=False):
    result = []
    def add(name, ready, required=True):
        result.append({"check": name, "ready": ready, "required": required})
    for plugin in ("playbin", "fakesink", "wavparse", "souphttpsrc", "autoaudiosink", "mpg123audiodec", "flacdec"):
        add("gstreamer:" + plugin, run(["gst-inspect-1.0", plugin]))
    add("secret-tool", shutil.which("secret-tool") is not None, False)
    add("session-dbus", run(["gdbus", "call", "--session", "--dest", "org.freedesktop.DBus", "--object-path", "/org/freedesktop/DBus", "--method", "org.freedesktop.DBus.ListNames"]), False)
    if build:
        for package in ("gstreamer-1.0", "gtk+-3.0", "webkit2gtk-4.1", "librsvg-2.0"):
            add("pkg-config:" + package, run(["pkg-config", "--exists", package]))
        for tool in ("cargo", "rustc", "cc"):
            add(tool, shutil.which(tool) is not None)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", action="store_true")
    args = parser.parse_args()
    result = checks(args.build)
    print(json.dumps({"checks": result, "frontendDistReady": (Path(__file__).resolve().parents[2] / "frontend/dist/index.html").is_file()}, ensure_ascii=False, indent=2))
    return 0 if all(row["ready"] or not row["required"] for row in result) else 1


if __name__ == "__main__":
    sys.exit(main())
