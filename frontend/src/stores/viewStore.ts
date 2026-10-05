import { createStore } from './createStore';
import type { BackendStatus } from '../types/backend';
import * as api from '../services/api';

export type ViewType = 'discover' | 'search' | 'playlist' | 'liked' | 'daily' | 'settings';

interface ViewState {
  currentView: ViewType;
  history: ViewType[];
  historyIndex: number;
  activePlaylistId: string | null;
  searchQuery: string;
  isLyricsOpen: boolean;
  isQueueOpen: boolean;
  backendStatus: BackendStatus | null;
}

const store = createStore<ViewState>({
  currentView: 'discover',
  history: ['discover'],
  historyIndex: 0,
  activePlaylistId: null,
  searchQuery: '',
  isLyricsOpen: false,
  isQueueOpen: false,
  backendStatus: null,
});

export const useViewStore = store.useStore;

export async function initViewSubscription() {
  api.getBackendStatus()
    .then((status) => store.setState({ backendStatus: status }))
    .catch((err) => console.warn('getBackendStatus failed:', err));

  return api.onBackendWarning((warning) => {
    console.warn('Backend warning:', warning);
  });
}

export const viewActions = {
  navigate(view: ViewType, playlistId?: string) {
    const current = store.getState();
    const newHistory = current.history.slice(0, current.historyIndex + 1);
    newHistory.push(view);

    store.setState({
      currentView: view,
      history: newHistory,
      historyIndex: newHistory.length - 1,
      activePlaylistId: playlistId ?? current.activePlaylistId,
    });
  },

  openPlaylist(playlistId: string) {
    const current = store.getState();
    const newHistory = current.history.slice(0, current.historyIndex + 1);
    newHistory.push('playlist');

    store.setState({
      currentView: 'playlist',
      activePlaylistId: playlistId,
      history: newHistory,
      historyIndex: newHistory.length - 1,
    });
  },

  back() {
    const current = store.getState();
    if (current.historyIndex > 0) {
      const nextIndex = current.historyIndex - 1;
      store.setState({
        currentView: current.history[nextIndex],
        historyIndex: nextIndex,
      });
    }
  },

  forward() {
    const current = store.getState();
    if (current.historyIndex < current.history.length - 1) {
      const nextIndex = current.historyIndex + 1;
      store.setState({
        currentView: current.history[nextIndex],
        historyIndex: nextIndex,
      });
    }
  },

  setSearchQuery(q: string) {
    store.setState({ searchQuery: q });
  },

  toggleLyrics() {
    store.setState((prev) => ({ isLyricsOpen: !prev.isLyricsOpen }));
  },

  setLyricsOpen(open: boolean) {
    store.setState({ isLyricsOpen: open });
  },

  toggleQueue() {
    store.setState((prev) => ({ isQueueOpen: !prev.isQueueOpen }));
  },

  setQueueOpen(open: boolean) {
    store.setState({ isQueueOpen: open });
  },
};
