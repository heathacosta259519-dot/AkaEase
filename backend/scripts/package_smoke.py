#!/usr/bin/env python3
"""Launch an extracted release in a private D-Bus session and temporary XDG directories.
Starts a private bus without service activation directories. Secret Service is unavailable.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('binary', type=Path)
args = parser.parse_args()
binary = args.binary.resolve()
with tempfile.TemporaryDirectory(prefix='aka-release-smoke-') as temp:
    root = Path(temp)
    shim = root / 'bin'
    shim.mkdir()
    secret_tool = shim / 'secret-tool'
    secret_tool.write_text('#!/bin/sh\nexit 1\n')
    secret_tool.chmod(0o700)
    env = os.environ.copy()
    env.update(XDG_CONFIG_HOME=str(root / 'config'), XDG_STATE_HOME=str(root / 'state'), XDG_CACHE_HOME=str(root / 'cache'))
    env['PATH'] = str(shim) + os.pathsep + env.get('PATH', '')
    config = root / 'bus.conf'
    config.write_text(f'<busconfig><type>session</type><listen>unix:path={root}/bus</listen><policy context="default"><allow send_destination="*"/><allow eavesdrop="true"/><allow own="*"/></policy></busconfig>')
    bus = subprocess.Popen(['dbus-daemon', '--nofork', '--print-address=1', '--config-file=' + str(config)], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    env['DBUS_SESSION_BUS_ADDRESS'] = bus.stdout.readline().strip()
    env['GTK_IM_MODULE'] = 'gtk-im-context-simple'
    env['NO_AT_BRIDGE'] = '1'
    env['GIO_USE_VFS'] = 'local'
    with (root / 'stderr.txt').open('w') as stderr:
        process = subprocess.Popen([str(binary)], env=env, stdout=subprocess.DEVNULL, stderr=stderr)
        try:
            log = root / 'state/akanetease/backend.jsonl'
            for _ in range(100):
                if process.poll() is not None:
                    raise RuntimeError('packaged host exited during startup')
                if log.exists() and 'mpris_ready' in log.read_text():
                    break
                time.sleep(0.1)
            else:
                raise RuntimeError('packaged host did not register MPRIS')
            reply = subprocess.run(['gdbus','call','--session','--dest','org.mpris.MediaPlayer2.AkaNetease','--object-path','/org/mpris/MediaPlayer2','--method','org.freedesktop.DBus.Properties.Get','org.mpris.MediaPlayer2.Player','CanControl'], capture_output=True, text=True, timeout=5, check=True, env=env)
            assert 'true' in reply.stdout
            time.sleep(6)
            process.terminate()
            assert process.wait(timeout=10) == 0
            events = [json.loads(line)['event'] for line in log.read_text().splitlines()]
            assert 'stopped' in events
            state = json.loads((root / 'state/akanetease/player.json').read_text())
            assert state['version'] == 1 and state['tracks'] == []
            print(json.dumps({'ok':True,'checks':['packaged host started','private MPRIS control registered','SIGTERM exited successfully','player state saved before exit'], 'keyring':'intentionally unavailable; real credentials not accessed'},indent=2))
        finally:
            bus.terminate()
            bus.wait(timeout=5)
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
