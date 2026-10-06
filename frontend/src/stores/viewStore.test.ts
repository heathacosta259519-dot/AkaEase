import { describe, it, expect, beforeEach } from 'vitest';
import { getViewState, viewActions } from './viewStore';

describe('viewStore navigation & history stack', () => {
  beforeEach(() => {
    viewActions._resetForTesting();
  });

  it('1. Initial state is discover view with zero-index history', () => {
    const state = getViewState();
    expect(state.currentView).toBe('discover');
    expect(state.historyIndex).toBe(0);
    expect(state.history).toHaveLength(1);
    expect(state.activeArtistId).toBeNull();
    expect(state.activeAlbumId).toBeNull();
    expect(state.activePlaylistId).toBeNull();
  });

  it('2. openArtist and openAlbum update view and respective entity IDs', () => {
    viewActions.openArtist('art-101');
    let state = getViewState();
    expect(state.currentView).toBe('artist');
    expect(state.activeArtistId).toBe('art-101');
    expect(state.activeAlbumId).toBeNull();

    viewActions.openAlbum('alb-202');
    state = getViewState();
    expect(state.currentView).toBe('album');
    expect(state.activeAlbumId).toBe('alb-202');
    expect(state.activeArtistId).toBeNull();

    // Back to artist
    viewActions.back();
    state = getViewState();
    expect(state.currentView).toBe('artist');
    expect(state.activeArtistId).toBe('art-101');
    expect(state.activeAlbumId).toBeNull();

    // Forward to album
    viewActions.forward();
    state = getViewState();
    expect(state.currentView).toBe('album');
    expect(state.activeAlbumId).toBe('alb-202');
  });

  it('3. History back and forward accurately restores entity IDs across playlist, album, and artist views', () => {
    viewActions.openPlaylist('pl-1');
    viewActions.openAlbum('alb-1');
    viewActions.openArtist('art-1');

    expect(getViewState().currentView).toBe('artist');
    expect(getViewState().activeArtistId).toBe('art-1');

    // Back to album
    viewActions.back();
    expect(getViewState().currentView).toBe('album');
    expect(getViewState().activeAlbumId).toBe('alb-1');
    expect(getViewState().activeArtistId).toBeNull();

    // Back to playlist
    viewActions.back();
    expect(getViewState().currentView).toBe('playlist');
    expect(getViewState().activePlaylistId).toBe('pl-1');
    expect(getViewState().activeAlbumId).toBeNull();

    // Back to initial discover
    viewActions.back();
    expect(getViewState().currentView).toBe('discover');

    // Forward to playlist
    viewActions.forward();
    expect(getViewState().currentView).toBe('playlist');
    expect(getViewState().activePlaylistId).toBe('pl-1');

    // Forward to album
    viewActions.forward();
    expect(getViewState().currentView).toBe('album');
    expect(getViewState().activeAlbumId).toBe('alb-1');

    // Forward to artist
    viewActions.forward();
    expect(getViewState().currentView).toBe('artist');
    expect(getViewState().activeArtistId).toBe('art-1');
  });
});
