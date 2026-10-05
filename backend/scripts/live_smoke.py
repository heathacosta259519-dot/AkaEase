#!/usr/bin/env python3
"""Opt-in, read-only probe through the compiled CLI; never part of cargo test."""

import json
from pathlib import Path
import subprocess

BINARY = Path(__file__).resolve().parents[1] / "target/debug/aka-backend"


def query(*args, allow_unavailable=False):
    result = subprocess.run(
        [str(BINARY), *args], capture_output=True, text=True, timeout=50
    )
    if result.returncode:
        error = json.loads(result.stderr)
        if allow_unavailable and error.get("code") == "unavailable":
            return None
        raise RuntimeError(f"{args[0]} failed: {error}")
    return json.loads(result.stdout)


def main():
    search = query("search", "Beyond")
    assert search["items"], "search returned no tracks"
    print(f"search: {len(search['items'])} items, total={search['total']}", flush=True)
    track_id = search["items"][0]["id"]
    tracks = query("track", track_id)
    assert tracks and tracks[0]["id"] == track_id
    print("track: identity and metadata present", flush=True)
    playlist = query("playlist", "3778678", "20")
    assert playlist["tracks"]["offset"] == 20 and playlist["tracks"]["items"]
    print(
        f"playlist: {len(playlist['tracks']['items'])} items at offset 20, "
        f"missing={len(playlist['unavailableIds'])}", flush=True
    )
    lyrics = query("lyrics", track_id)
    assert isinstance(lyrics, list)
    assert all(a["timeMs"] <= b["timeMs"] for a, b in zip(lyrics, lyrics[1:]))
    print(f"lyrics: {len(lyrics)} ordered lines", flush=True)
    stream_id = playlist["tracks"]["items"][0]["id"]
    stream = query("stream", stream_id, allow_unavailable=True)
    if stream is None:
        print("stream: anonymous session unavailable (no playback URL verified)", flush=True)
    else:
        assert stream["trackId"] == stream_id and stream["url"].startswith(("http://", "https://"))
        print(f"stream: URL resolved, preview={stream['isPreview']} (audio not downloaded)", flush=True)


if __name__ == "__main__":
    main()
