#!/usr/bin/env python3
"""Test the delivered frontend in real WebKit with synthetic audio and no keyring access."""
import json
import os
from pathlib import Path
import subprocess
import tempfile

backend = Path(__file__).resolve().parents[1]
if not (backend.parent / "frontend/dist/index.html").is_file():
    raise SystemExit("Build the real frontend dist before WebView acceptance.")
subprocess.run(["cargo", "build", "--locked", "--offline", "--manifest-path", str(backend / "desktop/Cargo.toml"), "--bin", "webview-check", "--features", "webview-check"], check=True)
with tempfile.TemporaryDirectory(prefix="aka-webview-check-") as temp:
    env = os.environ.copy()
    env["AKA_WEBVIEW_CHECK_DIR"] = temp
    env["XDG_CONFIG_HOME"] = temp + "/xdg-config"
    env["XDG_STATE_HOME"] = temp + "/xdg-state"
    env["XDG_CACHE_HOME"] = temp + "/xdg-cache"
    result = subprocess.run([str(backend / "desktop/target/debug/webview-check")], env=env, timeout=60)
    report = Path(temp) / "result.json"
    if report.exists():
        print(json.dumps(json.loads(report.read_text()), indent=2, ensure_ascii=False))
    else:
        print("No WebView acceptance report produced.")
    raise SystemExit(result.returncode)
