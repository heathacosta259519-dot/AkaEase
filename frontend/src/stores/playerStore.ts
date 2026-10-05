import { createStore } from './createStore';
import type { PlayerSnapshot, RepeatMode, Track } from '../types/backend';
import * as api from '../services/api';

interface PlayerState {
  snapshot: PlayerSnapshot | null;
  positionMs: number;
  lastSequence: bigint;
  isScrubbing: boolean;
  scrubPositionMs: number;
}

const store = createStore<PlayerState>({
  snapshot: null,
  positionMs: 0,
  lastSequence: 0n,
  isScrubbing: false,
  scrubPositionMs: 0,
});

let positionUpdatedAt = performance.now();

export function getCurrentPositionMs() {
  const { snapshot, positionMs, isScrubbing } = store.getState();
  if (snapshot?.playback.state !== 'playing' || isScrubbing) return positionMs;
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

  positionUpdatedAt = performance.now();
  store.setState({
    snapshot: snap,
    lastSequence: seq,
    positionMs: current.isScrubbing ? current.positionMs : snap.playback.positionMs,
  });
}

export const usePlayerStore = store.useStore;

export const playerActions = {
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
    positionUpdatedAt = performance.now();
    store.setState({ isScrubbing: false, positionMs: targetMs });
    try {
      const updated = await api.seekTo(Math.round(targetMs), snap.selectionId);
      applySnapshot(updated);
    } catch (err) {
      console.error('seek failed:', err);
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
    try {
      const snap = await api.replaceQueue(tracks, selected, autoplay);
      applySnapshot(snap);
    } catch (err) {
      console.error('replaceQueue failed:', err);
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
