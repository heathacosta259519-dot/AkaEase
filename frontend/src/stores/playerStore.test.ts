import { describe, it, expect, vi, beforeEach } from 'vitest';
import * as api from '../services/api';
import {
  playerActions,
  getPlayerState,
  getCurrentPositionMs,
  initPlayerSubscription,
} from './playerStore';
import type { PlayerSnapshot, PlaylistPage, Track } from '../types/backend';

vi.mock('../services/api');

function createMockSnapshot(overrides: Partial<PlayerSnapshot> = {}): PlayerSnapshot {
  return {
    sequence: '10',
    queueRevision: 'rev-1',
    selectionId: 'sel-1',
    current: {
      id: 'song-1',
      title: 'Test Song',
      artists: [{ id: 'art-1', name: 'Artist' }],
      album: { id: 'alb-1', name: 'Album', coverUrl: null },
      durationMs: 240000,
    },
    currentIndex: 0,
    queueLength: 1,
    playback: {
      state: 'playing',
      positionMs: 30000,
      durationMs: 240000,
      volume: 1,
    },
    playWhenReady: true,
    resolving: false,
    repeat: 'off',
    shuffle: false,
    canNext: true,
    canPrevious: false,
    canSeek: true,
    targetQuality: 'exhigh',
    actualQuality: 'exhigh',
    actualBitrate: 320000,
    format: 'mp3',
    bufferingPercent: null,
    isPreview: false,
    lastError: null,
    seekSerial: '0',
    seekPositionMs: 0,
    ...overrides,
  };
}

describe('playerStore seek and scrubbing anti-glitch protection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    playerActions._resetForTesting();
  });

  it('1. Dragging scrubbing keeps local scrub position and does not get overwritten by background snapshots', async () => {
    let listener!: (snap: PlayerSnapshot) => void;
    vi.mocked(api.onPlayerState).mockImplementation((cb) => {
      listener = cb;
      return Promise.resolve(() => {});
    });
    vi.mocked(api.getPlayerSnapshot).mockResolvedValue(createMockSnapshot({ sequence: '1' }));

    initPlayerSubscription();
    await Promise.resolve();

    // Start scrubbing to 120s
    playerActions.startScrubbing(120000);
    expect(getPlayerState().isScrubbing).toBe(true);
    expect(getPlayerState().scrubPositionMs).toBe(120000);
    expect(getCurrentPositionMs()).toBe(120000);

    // Backend pushes snapshot with old position 32s while scrubbing
    listener(createMockSnapshot({ sequence: '2', playback: { state: 'playing', positionMs: 32000, durationMs: 240000, volume: 1 } }));

    // Position during scrubbing must remain at 120s!
    expect(getPlayerState().positionMs).toBe(120000);
    expect(getCurrentPositionMs()).toBe(120000);
  });

  it('2. Mouse release (seek): prevents zero-drop glitch when GStreamer flushes or emits transient 0 position', async () => {
    let listener!: (snap: PlayerSnapshot) => void;
    vi.mocked(api.onPlayerState).mockImplementation((cb) => {
      listener = cb;
      return Promise.resolve(() => {});
    });
    vi.mocked(api.getPlayerSnapshot).mockResolvedValue(createMockSnapshot({ sequence: '1' }));

    initPlayerSubscription();
    await Promise.resolve();

    let resolveSeek!: (snap: PlayerSnapshot) => void;
    vi.mocked(api.seekTo).mockReturnValue(
      new Promise<PlayerSnapshot>((res) => {
        resolveSeek = res;
      })
    );

    // User drags and releases at 150s (150000ms)
    const targetMs = 150000;
    playerActions.seek(targetMs);

    // Optimistically set to 150s immediately upon mouseup
    expect(getPlayerState().isScrubbing).toBe(false);
    expect(getPlayerState().positionMs).toBe(targetMs);

    // Backend GStreamer flushes and transiently emits positionMs = 0 in event stream!
    listener(
      createMockSnapshot({
        sequence: '2',
        playback: { state: 'playing', positionMs: 0, durationMs: 240000, volume: 1 },
      })
    );

    // Must NOT drop to 0!
    expect(getPlayerState().positionMs).toBeGreaterThanOrEqual(targetMs);
    expect(getCurrentPositionMs()).toBeGreaterThanOrEqual(targetMs);

    // Backend also emits stale pre-seek position 35s
    listener(
      createMockSnapshot({
        sequence: '3',
        playback: { state: 'playing', positionMs: 35000, durationMs: 240000, volume: 1 },
      })
    );

    // Still must NOT jump back to 35s!
    expect(getPlayerState().positionMs).toBeGreaterThanOrEqual(targetMs);

    // Finally, backend seek completes and reports settled position near 150s
    resolveSeek(
      createMockSnapshot({
        sequence: '4',
        seekSerial: '1',
        seekPositionMs: targetMs,
        playback: { state: 'playing', positionMs: 150100, durationMs: 240000, volume: 1 },
      })
    );
    await Promise.resolve();

    // Now accepted settled position
    expect(getPlayerState().positionMs).toBe(150100);
  });
});

function tracksFrom(start: number, count: number): Track[] {
  return Array.from({ length: count }, (_, index) => ({
    ...createMockSnapshot().current!,
    id: String(start + index),
  }));
}

function playlistPage(offset: number, items: Track[], total = 205): PlaylistPage {
  return {
    id: '123', title: 'Playlist', unavailableIds: [],
    tracks: { items, total, offset, hasMore: offset + 100 < total },
  };
}

describe('background playlist playback', () => {
  beforeEach(() => {
    vi.resetAllMocks();
    playerActions._resetForTesting();
    vi.mocked(api.replaceQueue).mockResolvedValue(createMockSnapshot());
    vi.mocked(api.expandQueue).mockResolvedValue(createMockSnapshot());
  });

  it('starts the selected song immediately and progressively queues all 205 songs', async () => {
    const tracks = tracksFrom(1, 205);
    vi.mocked(api.getPlaylist).mockImplementation(async (_id, offset = 0) =>
      playlistPage(offset, tracks.slice(offset, offset + 100)));

    await playerActions.replacePlaylistQueue('123', tracks.slice(0, 100), '57');

    expect(vi.mocked(api.getPlaylist).mock.calls).toEqual([
      ['123', 0, 100], ['123', 100, 100], ['123', 200, 100],
    ]);
    expect(api.replaceQueue).toHaveBeenCalledExactlyOnceWith(tracks.slice(0, 100), 56, true);
    expect(api.expandQueue).toHaveBeenLastCalledWith(tracks, 'rev-1');
    expect(api.expandQueue).toHaveBeenCalledTimes(3);
    expect(getPlayerState().loadingPlaylistId).toBeNull();
  });

  it('advances over missing songs and empty pages without losing later songs or duplicates', async () => {
    const first = tracksFrom(1, 99);
    const last = [first[0], ...tracksFrom(201, 4)];
    vi.mocked(api.getPlaylist)
      .mockResolvedValueOnce(playlistPage(0, first))
      .mockResolvedValueOnce(playlistPage(100, []))
      .mockResolvedValueOnce(playlistPage(200, last));

    await playerActions.replacePlaylistQueue('123', [...first, ...last], '201');

    expect(api.getPlaylist).toHaveBeenNthCalledWith(3, '123', 200, 100);
    expect(api.expandQueue).toHaveBeenCalledExactlyOnceWith([...first, ...last], 'rev-1');
  });

  it('play all selects the first song from the complete playlist', async () => {
    const tracks = tracksFrom(1, 101);
    vi.mocked(api.getPlaylist)
      .mockResolvedValueOnce(playlistPage(0, tracks.slice(0, 100), 101))
      .mockResolvedValueOnce(playlistPage(100, tracks.slice(100), 101));

    await playerActions.replacePlaylistQueue('123', tracks.slice(0, 100));

    expect(api.replaceQueue).toHaveBeenCalledExactlyOnceWith(tracks.slice(0, 100), 0, true);
    expect(api.expandQueue).toHaveBeenLastCalledWith(tracks, 'rev-1');
  });

  it('retains immediate playback after a later page failure, then allows retry', async () => {
    vi.mocked(api.getPlaylist)
      .mockResolvedValueOnce(playlistPage(0, tracksFrom(1, 100)))
      .mockRejectedValueOnce({ code: 'network' });

    const initial = tracksFrom(1, 100);
    await playerActions.replacePlaylistQueue('123', initial, '57');

    expect(api.replaceQueue).toHaveBeenCalledExactlyOnceWith(initial, 56, true);
    expect(getPlayerState().playlistError?.id).toBe('123');
    expect(getPlayerState().loadingPlaylistId).toBeNull();

    vi.mocked(api.getPlaylist).mockResolvedValue(playlistPage(0, tracksFrom(1, 1), 1));
    await playerActions.replacePlaylistQueue('123', tracksFrom(1, 1));
    expect(api.replaceQueue).toHaveBeenCalledTimes(2);
    expect(getPlayerState().playlistError).toBeNull();
  });

  it('reports a changed playlist while retaining playback and rejects invalid initial selection', async () => {
    vi.mocked(api.getPlaylist)
      .mockResolvedValueOnce(playlistPage(0, tracksFrom(1, 100)))
      .mockResolvedValueOnce(playlistPage(100, tracksFrom(101, 10), 110));
    await playerActions.replacePlaylistQueue('123', tracksFrom(1, 100));
    expect(api.expandQueue).toHaveBeenCalledOnce();
    expect(getPlayerState().playlistError?.message).toContain('变化');

    vi.mocked(api.getPlaylist).mockResolvedValue(playlistPage(0, tracksFrom(1, 1), 1));
    await playerActions.replacePlaylistQueue('123', tracksFrom(1, 1), '57');
    expect(api.replaceQueue).toHaveBeenCalledOnce();
    expect(getPlayerState().playlistError?.message).toContain('不可用');
  });

  it('ignores a slow previous double click when a newer song was selected', async () => {
    let finishOld!: (page: PlaylistPage) => void;
    vi.mocked(api.getPlaylist).mockReturnValueOnce(new Promise((resolve) => { finishOld = resolve; }));
    const oldRequest = playerActions.replacePlaylistQueue('123', tracksFrom(1, 2), '1');
    await Promise.resolve();
    const tracks = tracksFrom(1, 2);
    vi.mocked(api.getPlaylist).mockResolvedValue(playlistPage(0, tracks, 2));
    await playerActions.replacePlaylistQueue('123', tracks, '2');
    finishOld(playlistPage(0, tracks, 2));
    await oldRequest;

    expect(api.replaceQueue).toHaveBeenCalledTimes(2);
    expect(api.replaceQueue).toHaveBeenLastCalledWith(tracks, 1, true);
    expect(api.expandQueue).toHaveBeenCalledOnce();
  });

  it('ignores an unfinished playlist load after another queue is selected', async () => {
    let finish!: (page: PlaylistPage) => void;
    vi.mocked(api.getPlaylist).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const request = playerActions.replacePlaylistQueue('123', tracksFrom(1, 100));
    await Promise.resolve();
    const otherTracks = tracksFrom(300, 2);
    await playerActions.replaceQueue(otherTracks, 1, true);
    finish(playlistPage(0, tracksFrom(1, 100)));
    await request;

    expect(api.replaceQueue).toHaveBeenLastCalledWith(otherTracks, 1, true);
    expect(api.expandQueue).not.toHaveBeenCalled();
    expect(getPlayerState().loadingPlaylistId).toBeNull();
  });

  it('keeps loading after playback controls or MPRIS selection changes', async () => {
    let finish!: (page: PlaylistPage) => void;
    let listener!: (snap: PlayerSnapshot) => void;
    vi.mocked(api.getPlayerSnapshot).mockResolvedValue(createMockSnapshot({ sequence: '1' }));
    vi.mocked(api.onPlayerState).mockImplementation(async (callback) => {
      listener = callback;
      return () => {};
    });
    await initPlayerSubscription();
    vi.mocked(api.getPlaylist).mockReturnValue(new Promise((resolve) => { finish = resolve; }));
    const initial = tracksFrom(1, 100);
    const request = playerActions.replacePlaylistQueue('123', initial);
    await Promise.resolve();
    vi.mocked(api.pause).mockResolvedValue(createMockSnapshot({ sequence: '11' }));
    await playerActions.pause();
    listener(createMockSnapshot({ sequence: '2', selectionId: 'sel-2' }));
    finish(playlistPage(0, initial, 100));
    await request;

    expect(api.expandQueue).toHaveBeenCalledExactlyOnceWith(initial, 'rev-1');
  });

  it('plays before a slow first page completes even for a 2005-song playlist', async () => {
    let finish!: (page: PlaylistPage) => void;
    const initial = tracksFrom(1, 100);
    vi.mocked(api.getPlaylist).mockReturnValueOnce(new Promise((resolve) => { finish = resolve; }));
    const request = playerActions.replacePlaylistQueue('123', initial, '57');
    await Promise.resolve();
    expect(api.replaceQueue).toHaveBeenCalledExactlyOnceWith(initial, 56, true);
    expect(getPlayerState().snapshot?.playWhenReady).toBe(true);
    expect(api.expandQueue).not.toHaveBeenCalled();
    vi.mocked(api.getPlaylist).mockImplementation(async (_id, offset = 0) =>
      playlistPage(offset, tracksFrom(offset + 1, Math.min(100, 2005 - offset)), 2005));
    finish(playlistPage(0, initial, 2005));
    await request;
    expect(api.expandQueue).toHaveBeenCalledTimes(21);
    expect(vi.mocked(api.expandQueue).mock.calls.at(-1)?.[0]).toHaveLength(2005);
  });

  it('passes the latest returned revision and stops on a stale backend queue', async () => {
    const initial = tracksFrom(1, 100);
    vi.mocked(api.getPlaylist).mockImplementation(async (_id, offset = 0) =>
      playlistPage(offset, tracksFrom(offset + 1, Math.min(100, 205 - offset))));
    vi.mocked(api.expandQueue)
      .mockResolvedValueOnce(createMockSnapshot({ sequence: '11', queueRevision: 'rev-2' }))
      .mockRejectedValueOnce({ code: 'stale_operation' });
    await playerActions.replacePlaylistQueue('123', initial);
    expect(api.expandQueue).toHaveBeenNthCalledWith(2, tracksFrom(1, 200), 'rev-2');
    expect(api.getPlaylist).toHaveBeenCalledTimes(2);
    expect(getPlayerState().playlistError).toBeNull();
    expect(getPlayerState().loadingPlaylistId).toBeNull();
  });

  it('updates snapshot when setQuality is called', async () => {
    const updated = createMockSnapshot({
      sequence: '12',
      targetQuality: 'lossless',
      actualQuality: 'lossless',
      actualBitrate: 999000,
      format: 'flac',
    });
    vi.mocked(api.setQuality).mockResolvedValue(updated);
    await playerActions.setQuality('lossless');
    expect(api.setQuality).toHaveBeenCalledWith('lossless');
    expect(getPlayerState().snapshot?.targetQuality).toBe('lossless');
    expect(getPlayerState().snapshot?.actualQuality).toBe('lossless');
    expect(getPlayerState().snapshot?.actualBitrate).toBe(999000);
    expect(getPlayerState().snapshot?.format).toBe('flac');
  });
});
