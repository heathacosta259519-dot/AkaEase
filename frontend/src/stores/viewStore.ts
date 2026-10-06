import { createStore } from './createStore';
import type { BackendStatus } from '../types/backend';
import * as api from '../services/api';

export type ViewType =
  | 'discover'
  | 'search'
  | 'playlist'
  | 'liked'
  | 'daily'
  | 'settings'
  | 'artist'
  | 'album';

export interface HistoryEntry {
  view: ViewType;
  playlistId: string | null;
  artistId: string | null;
  albumId: string | null;
}

interface ViewState {
  currentView: ViewType;
  history: HistoryEntry[];
  historyIndex: number;
  activePlaylistId: string | null;
  activeArtistId: string | null;
  activeAlbumId: string | null;
  searchQuery: string;
  isLyricsOpen: boolean;
  isQueueOpen: boolean;
  backendStatus: BackendStatus | null;
}

const initialEntry: HistoryEntry = {
  view: 'discover',
  playlistId: null,
  artistId: null,
  albumId: null,
};

const store = createStore<ViewState>({
  currentView: 'discover',
  history: [initialEntry],
  historyIndex: 0,
  activePlaylistId: null,
  activeArtistId: null,
  activeAlbumId: null,
  searchQuery: '',
  isLyricsOpen: false,
  isQueueOpen: false,
  backendStatus: null,
});

export const useViewStore = store.useStore;
export const getViewState = store.getState;

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
    const newEntry: HistoryEntry = {
      view,
      playlistId: playlistId ?? current.activePlaylistId,
      artistId: view === 'artist' ? current.activeArtistId : null,
      albumId: view === 'album' ? current.activeAlbumId : null,
    };
    const newHistory = current.history.slice(0, current.historyIndex + 1);
    newHistory.push(newEntry);

    store.setState({
      currentView: view,
      history: newHistory,
      historyIndex: newHistory.length - 1,
      activePlaylistId: newEntry.playlistId,
    });
  },

  openPlaylist(playlistId: string) {
    const current = store.getState();
    const newEntry: HistoryEntry = {
      view: 'playlist',
      playlistId,
      artistId: null,
      albumId: null,
    };
    const newHistory = current.history.slice(0, current.historyIndex + 1);
    newHistory.push(newEntry);

    store.setState({
      currentView: 'playlist',
      activePlaylistId: playlistId,
      activeArtistId: null,
      activeAlbumId: null,
      history: newHistory,
      historyIndex: newHistory.length - 1,
    });
  },

  openArtist(artistId: string) {
    const current = store.getState();
    const newEntry: HistoryEntry = {
      view: 'artist',
      playlistId: null,
      artistId,
      albumId: null,
    };
    const newHistory = current.history.slice(0, current.historyIndex + 1);
    newHistory.push(newEntry);

    store.setState({
      currentView: 'artist',
      activeArtistId: artistId,
      activePlaylistId: null,
      activeAlbumId: null,
      history: newHistory,
      historyIndex: newHistory.length - 1,
    });
  },

  openAlbum(albumId: string) {
    const current = store.getState();
    const newEntry: HistoryEntry = {
      view: 'album',
      playlistId: null,
      artistId: null,
      albumId,
    };
    const newHistory = current.history.slice(0, current.historyIndex + 1);
    newHistory.push(newEntry);

    store.setState({
      currentView: 'album',
      activeAlbumId: albumId,
      activePlaylistId: null,
      activeArtistId: null,
      history: newHistory,
      historyIndex: newHistory.length - 1,
    });
  },

  back() {
    const current = store.getState();
    if (current.historyIndex > 0) {
      const nextIndex = current.historyIndex - 1;
      const target = current.history[nextIndex];
      store.setState({
        currentView: target.view,
        activePlaylistId: target.playlistId,
        activeArtistId: target.artistId,
        activeAlbumId: target.albumId,
        historyIndex: nextIndex,
      });
    }
  },

  forward() {
    const current = store.getState();
    if (current.historyIndex < current.history.length - 1) {
      const nextIndex = current.historyIndex + 1;
      const target = current.history[nextIndex];
      store.setState({
        currentView: target.view,
        activePlaylistId: target.playlistId,
        activeArtistId: target.artistId,
        activeAlbumId: target.albumId,
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

  _resetForTesting() {
    store.setState({
      currentView: 'discover',
      history: [initialEntry],
      historyIndex: 0,
      activePlaylistId: null,
      activeArtistId: null,
      activeAlbumId: null,
      searchQuery: '',
      isLyricsOpen: false,
      isQueueOpen: false,
      backendStatus: null,
    });
  },
};
