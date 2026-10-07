import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AlbumDetail,
  AlbumSummary,
  AppConfig,
  ArtistDetail,
  BackendStatus,
  CacheStats,
  LyricLine,
  LogoutReport,
  Page,
  PlayerQueue,
  PlayerSnapshot,
  PlaylistPage,
  QrChallenge,
  QrProgress,
  RepeatMode,
  SessionEventPayload,
  SessionSnapshot,
  SoundQuality,
  Track,
  UserPlaylist,
  UserPlaylists,
} from '../types/backend';

// ==========================================
// 音乐与内容 API
// ==========================================

export async function searchMusic(query: string, offset = 0, limit = 20): Promise<Page<Track>> {
  return invoke('music_search', { query, offset, limit });
}

export async function getTracks(ids: string[]): Promise<Track[]> {
  return invoke('music_tracks', { ids });
}

export async function getPlaylist(id: string, offset = 0, limit = 50): Promise<PlaylistPage> {
  return invoke('music_playlist', { id, offset, limit });
}

export async function getArtistDetail(id: string): Promise<ArtistDetail> {
  return invoke('music_artist_detail', { id });
}

export async function getArtistSongs(id: string): Promise<Track[]> {
  return invoke('music_artist_songs', { id });
}

export async function getArtistAlbums(id: string, offset = 0, limit = 30): Promise<Page<AlbumSummary>> {
  return invoke('music_artist_albums', { id, offset, limit });
}

export async function getAlbumDetail(id: string): Promise<AlbumDetail> {
  return invoke('music_album_detail', { id });
}

export async function getLyrics(id: string): Promise<LyricLine[]> {
  return invoke('music_lyrics', { id });
}

// ==========================================
// 账号与会话 API
// ==========================================

export async function loginBegin(): Promise<QrChallenge> {
  return invoke('login_begin');
}

export async function loginPoll(attemptId: string): Promise<QrProgress> {
  return invoke('login_poll', { attemptId });
}

export async function loginCancel(attemptId: string): Promise<void> {
  return invoke('login_cancel', { attemptId });
}

export async function restoreSession(): Promise<SessionSnapshot> {
  return invoke('session_restore');
}

export async function getSessionSnapshot(): Promise<SessionSnapshot> {
  return invoke('session_snapshot');
}

export async function refreshSession(): Promise<SessionSnapshot> {
  return invoke('session_refresh');
}

export async function logout(): Promise<LogoutReport> {
  return invoke('logout');
}

export async function getUserPlaylists(offset = 0, limit = 30): Promise<UserPlaylists> {
  return invoke('user_playlists', { offset, limit });
}

export async function getLikedTracks(): Promise<string[]> {
  return invoke('liked_tracks');
}

export async function getDailyTracks(): Promise<Track[]> {
  return invoke('daily_tracks');
}

export async function trackLike(trackId: string, like: boolean): Promise<boolean> {
  return invoke('track_like', { trackId, like });
}

export async function playlistCreate(name: string, privacy?: number): Promise<UserPlaylist> {
  return invoke('playlist_create', { name, privacy });
}

export async function playlistDelete(playlistId: string): Promise<void> {
  return invoke('playlist_delete', { playlistId });
}

export async function playlistTracksOp(
  playlistId: string,
  trackIds: string[],
  op: 'add' | 'del',
): Promise<number> {
  return invoke('playlist_tracks_op', { playlistId, trackIds, op });
}

export async function playlistSubscribe(playlistId: string, subscribe: boolean): Promise<void> {
  return invoke('playlist_subscribe', { playlistId, subscribe });
}

// ==========================================
// 播放器控制 API
// ==========================================

export async function getPlayerSnapshot(): Promise<PlayerSnapshot> {
  return invoke('player_snapshot');
}

export async function getPlayerQueue(): Promise<PlayerQueue> {
  return invoke('player_queue');
}

export async function replaceQueue(
  tracks: Track[],
  selected = 0,
  autoplay = true,
): Promise<PlayerSnapshot> {
  return invoke('player_replace', { tracks, selected, autoplay });
}

export async function selectQueueItem(index: number, revision: string): Promise<PlayerSnapshot> {
  return invoke('player_select', { index, revision });
}

export async function expandQueue(tracks: Track[], revision: string): Promise<PlayerSnapshot> {
  return invoke('player_expand', { tracks, revision });
}

export async function removeQueueItem(index: number, revision: string): Promise<PlayerSnapshot> {
  return invoke('player_remove', { index, revision });
}

export async function play(): Promise<PlayerSnapshot> {
  return invoke('player_play');
}

export async function pause(): Promise<PlayerSnapshot> {
  return invoke('player_pause');
}

export async function togglePlay(): Promise<PlayerSnapshot> {
  return invoke('player_toggle');
}

export async function stop(): Promise<PlayerSnapshot> {
  return invoke('player_stop');
}

export async function nextTrack(): Promise<PlayerSnapshot> {
  return invoke('player_next');
}

export async function previousTrack(): Promise<PlayerSnapshot> {
  return invoke('player_previous');
}

export async function seekTo(positionMs: number, selectionId: string): Promise<PlayerSnapshot> {
  return invoke('player_seek', { positionMs, selectionId });
}

export async function setVolume(volume: number): Promise<PlayerSnapshot> {
  return invoke('player_volume', { volume });
}

export async function setRepeat(repeat: RepeatMode): Promise<PlayerSnapshot> {
  return invoke('player_repeat', { repeat });
}

export async function setShuffle(shuffle: boolean): Promise<PlayerSnapshot> {
  return invoke('player_shuffle', { shuffle });
}

export async function setQuality(quality: SoundQuality): Promise<PlayerSnapshot> {
  return invoke('player_set_quality', { quality });
}

// ==========================================
// 诊断、配置与缓存 API
// ==========================================

export async function getBackendStatus(): Promise<BackendStatus> {
  return invoke('backend_status');
}

export async function getConfig(): Promise<AppConfig> {
  return invoke('config_get');
}

export async function setConfig(config: AppConfig): Promise<{ restartRequired: boolean }> {
  return invoke('config_set', { config });
}

export async function getCacheStats(): Promise<CacheStats> {
  return invoke('cache_stats');
}

export async function clearCache(): Promise<void> {
  return invoke('cache_clear');
}

// ==========================================
// 事件监听器
// ==========================================

export function onPlayerState(callback: (snapshot: PlayerSnapshot) => void): Promise<UnlistenFn> {
  return listen<PlayerSnapshot>('player-state', (event) => callback(event.payload));
}

export function onSessionState(callback: (event: SessionEventPayload) => void): Promise<UnlistenFn> {
  return listen<SessionEventPayload>('session-state', (event) => callback(event.payload));
}

export function onBackendWarning(callback: (warning: string) => void): Promise<UnlistenFn> {
  return listen<string>('backend-warning', (event) => callback(event.payload));
}
