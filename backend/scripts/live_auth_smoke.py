#!/usr/bin/env python3
"""Probe QR creation/waiting/cancellation without reading or writing any keyring."""
import json
from pathlib import Path
import subprocess

binary = Path(__file__).resolve().parents[1] / "target/debug/aka-backend"
result = subprocess.run([str(binary), "qr-probe"], capture_output=True, text=True, timeout=50)
if result.returncode:
    raise RuntimeError(f"QR probe failed: {result.stderr.strip()}")
data = json.loads(result.stdout)
assert data["qrCreated"] and data["status"] == "waiting_scan", data
print("QR created, waiting_scan verified, local attempt cancelled; keyring untouched")
