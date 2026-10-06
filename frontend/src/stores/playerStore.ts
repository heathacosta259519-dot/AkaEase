import { createStore } from './createStore';
import type { PlayerSnapshot, RepeatMode, Track } from '../types/backend';
import * as api from '../services/api';

interface PlayerState {
  snapshot: PlayerSnapshot | null;
  positionMs: number;
  lastSequence: bigint;
  isScrubbing: boolean;
  scrubPositionMs: number;
  loadingPlaylistId: string | null;
  playlistError: { id: string; message: string } | null;
}

interface PendingSeek {
  targetMs: number;
  initiatedAt: number;
  selectionId: string;
}

const store = createStore<PlayerState>({
  snapshot: null,
  positionMs: 0,
  lastSequence: 0n,
  isScrubbing: false,
  scrubPositionMs: 0,
  loadingPlaylistId: null,
  playlistError: null,
});

let positionUpdatedAt = performance.now();
let pendingSeek: PendingSeek | null = null;
let queueLoadGeneration = 0;

function cancelPlaylistLoad() {
  queueLoadGeneration++;
  store.setState({ loadingPlaylistId: null, playlistError: null });
}

export function getCurrentPositionMs() {
  const { snapshot, positionMs, isScrubbing, scrubPositionMs } = store.getState();
  if (isScrubbing) return scrubPositionMs;
  if (snapshot?.playback.state !== 'playing') return positionMs;
  const duration = snapshot.playback.durationMs ?? Infinity;
  return Math.min(duration, positionMs + Math.max(0, performance.now() - positionUpdatedAt));
}

export function initPlayerSubscription() {
  // Fetch initial snapshot
  api.getPlayerSnapshot()
    .then((snap) => {
      applySnapshot(snap);
    })
    .catch((err) => console.warn('Failed to get initial player snapshot:', err));

  // Listen to player-state events
  return api.onPlayerState((snap) => {
    applySnapshot(snap);
  });
}

function applySnapshot(snap: PlayerSnapshot) {
  const current = store.getState();
  const seq = BigInt(snap.sequence);
  if (seq < current.lastSequence) {
    return; // Discard outdated sequence
  }

  const now = performance.now();
  let nextPositionMs = snap.playback.positionMs;

  if (current.isScrubbing) {
    // 正在拖拽时，保留前端拖拽中的位置，避免后端推送打乱手势
    nextPositionMs = current.positionMs;
  } else if (pendingSeek) {
    const elapsed = now - pendingSeek.initiatedAt;
    // 如果切歌了（selectionId 不一致），pendingSeek 立即失效
    if (snap.selectionId !== pendingSeek.selectionId) {
      pendingSeek = null;
    } else if (elapsed < 1200) {
      // 在 seek 发起后的缓冲期内：
      // GStreamer 在 flush/seek 瞬间会上报 0 位置，或者 IPC 排队了 seek 前的旧快照事件
      const isFlushZero = snap.playback.positionMs === 0 && pendingSeek.targetMs >= 800;
      const isStalePosition = Math.abs(snap.playback.positionMs - pendingSeek.targetMs) > 1500;

      if (isFlushZero || (elapsed < 600 && isStalePosition)) {
        // 忽略后端的瞬态 0 或旧位置，保持乐观 seek 位置并随播放时间正常推进
        const duration = snap.playback.durationMs ?? Infinity;
        const playOffset = snap.playback.state === 'playing' ? Math.max(0, now - positionUpdatedAt) : 0;
        nextPositionMs = Math.min(duration, current.positionMs + playOffset);
      } else {
        // 后端上报位置已接近 seek 目标位置，说明 seek 已平滑到位
        pendingSeek = null;
      }
    } else {
      // 超过保护期，恢复信任后端位置
      pendingSeek = null;
    }
  }

  positionUpdatedAt = now;
  store.setState({
    snapshot: snap,
    lastSequence: seq,
    positionMs: nextPositionMs,
  });
}

export const usePlayerStore = store.useStore;
export const getPlayerState = store.getState;

export const playerActions = {
  _resetForTesting() {
    cancelPlaylistLoad();
    positionUpdatedAt = performance.now();
    pendingSeek = null;
    store.setState({
      snapshot: null,
      positionMs: 0,
      lastSequence: 0n,
      isScrubbing: false,
      scrubPositionMs: 0,
    });
  },
  async toggle() {
    try {
      const snap = await api.togglePlay();
      applySnapshot(snap);
    } catch (err) {
      console.error('togglePlay failed:', err);
    }
  },

  async play() {
    try {
      const snap = await api.play();
      applySnapshot(snap);
    } catch (err) {
      console.error('play failed:', err);
    }
  },

  async pause() {
    try {
      const snap = await api.pause();
      applySnapshot(snap);
    } catch (err) {
      console.error('pause failed:', err);
    }
  },

  async next() {
    try {
      const snap = await api.nextTrack();
      applySnapshot(snap);
    } catch (err) {
      console.error('nextTrack failed:', err);
    }
  },

  async previous() {
    try {
      const snap = await api.previousTrack();
      applySnapshot(snap);
    } catch (err) {
      console.error('previousTrack failed:', err);
    }
  },

  async seek(targetMs: number) {
    const snap = store.getState().snapshot;
    if (!snap) return;
    const duration = snap.playback.durationMs ?? targetMs;
    const clampedTarget = Math.max(0, Math.min(duration, targetMs));
    const now = performance.now();
    positionUpdatedAt = now;
    pendingSeek = {
      targetMs: clampedTarget,
      initiatedAt: now,
      selectionId: snap.selectionId,
    };
    store.setState({
      isScrubbing: false,
      positionMs: clampedTarget,
      scrubPositionMs: clampedTarget,
    });
    try {
      const updated = await api.seekTo(Math.round(clampedTarget), snap.selectionId);
      applySnapshot(updated);
    } catch (err) {
      console.error('seek failed:', err);
      if (pendingSeek?.initiatedAt === now) {
        pendingSeek = null;
      }
    }
  },

  startScrubbing(positionMs: number) {
    store.setState({ isScrubbing: true, scrubPositionMs: positionMs, positionMs });
  },

  updateScrubbing(positionMs: number) {
    store.setState({ scrubPositionMs: positionMs, positionMs });
  },

  async setVolume(vol: number) {
    const clamped = Math.max(0, Math.min(1, vol));
    try {
      const snap = await api.setVolume(clamped);
      applySnapshot(snap);
    } catch (err) {
      console.error('setVolume failed:', err);
    }
  },

  async setRepeat(repeat: RepeatMode) {
    try {
      const snap = await api.setRepeat(repeat);
      applySnapshot(snap);
    } catch (err) {
      console.error('setRepeat failed:', err);
    }
  },

  async setShuffle(shuffle: boolean) {
    try {
      const snap = await api.setShuffle(shuffle);
      applySnapshot(snap);
    } catch (err) {
      console.error('setShuffle failed:', err);
    }
  },

  async replaceQueue(tracks: Track[], selected = 0, autoplay = true) {
    cancelPlaylistLoad();
    try {
      const snap = await api.replaceQueue(tracks, selected, autoplay);
      applySnapshot(snap);
    } catch (err) {
      console.error('replaceQueue failed:', err);
    }
  },

  async replacePlaylistQueue(id: string, initialTracks: Track[], selectedTrackId?: string) {
    const generation = ++queueLoadGeneration;
    store.setState({ loadingPlaylistId: id, playlistError: null });

    try {
      const selected = selectedTrackId === undefined
        ? 0 : initialTracks.findIndex((track) => track.id === selectedTrackId);
      if (initialTracks.length === 0 || selected < 0) throw new Error('所选歌曲已不可用，请刷新歌单');
      const started = await api.replaceQueue(initialTracks, selected, true);
      if (generation !== queueLoadGeneration) return;
      applySnapshot(started);
      let revision = started.queueRevision;
      const tracks: Track[] = [];
      let total: number | undefined;
      // Advance by requested page size, even when a page contains unavailable songs.
      for (let offset = 0; ; offset += 100) {
        const page = await api.getPlaylist(id, offset, 100);
        if (generation !== queueLoadGeneration) return;
        if (page.id !== id || page.tracks.offset !== offset ||
            (total !== undefined && page.tracks.total !== total)) {
          throw new Error('歌单已变化，请重试');
        }
        total = page.tracks.total;
        tracks.push(...page.tracks.items);
        if (tracks.length >= initialTracks.length) {
          const expanded = await api.expandQueue([...tracks], revision);
          if (generation !== queueLoadGeneration) return;
          revision = expanded.queueRevision;
          applySnapshot(expanded);
        } else if (!page.tracks.hasMore) {
          throw new Error('歌单已变化，请刷新后重试');
        }
        if (!page.tracks.hasMore) break;
      }
    } catch (err) {
      if (generation === queueLoadGeneration &&
          !(typeof err === 'object' && err !== null && 'code' in err && err.code === 'stale_operation')) {
        store.setState({
          playlistError: {
            id,
            message: err instanceof Error ? err.message : '后台歌单加载失败，已加载歌曲仍可播放，请重试',
          },
        });
      }
    } finally {
      if (generation === queueLoadGeneration) {
        store.setState({ loadingPlaylistId: null });
      }
    }
  },

  async selectQueueItem(index: number, revision?: string) {
    const rev = revision ?? store.getState().snapshot?.queueRevision;
    if (!rev) return;
    try {
      const updated = await api.selectQueueItem(index, rev);
      applySnapshot(updated);
    } catch (err) {
      console.error('selectQueueItem failed:', err);
    }
  },

  async removeQueueItem(index: number, revision?: string) {
    cancelPlaylistLoad();
    const rev = revision ?? store.getState().snapshot?.queueRevision;
    if (!rev) return;
    try {
      const updated = await api.removeQueueItem(index, rev);
      applySnapshot(updated);
    } catch (err) {
      console.error('removeQueueItem failed:', err);
    }
  },
};
