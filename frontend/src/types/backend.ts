// ==========================================
// 基础模型 (Music Models)
// ==========================================

export interface Artist {
  id: string;
  name: string;
}

export interface Album {
  id: string;
  name: string;
  coverUrl: string | null;
}

export interface Track {
  id: string;
  title: string;
  artists: Artist[];
  album: Album;
  durationMs: number;
}

export interface Page<T> {
  items: T[];
  total: number;
  offset: number;
  hasMore: boolean;
}

export interface PlaylistPage {
  id: string;
  title: string;
  tracks: Page<Track>;
  unavailableIds: string[];
}

export interface ArtistDetail {
  id: string;
  name: string;
  coverUrl: string | null;
  aliases: string[];
  briefDescription: string | null;
  musicSize: number;
  albumSize: number;
}

export interface AlbumSummary {
  id: string;
  name: string;
  coverUrl: string | null;
  artist: Artist | null;
  artists: Artist[];
  publishTimeMs: number | null;
  trackCount: number;
}

export interface AlbumDetail {
  id: string;
  name: string;
  coverUrl: string | null;
  artist: Artist | null;
  artists: Artist[];
  description: string | null;
  publishTimeMs: number | null;
  company: string | null;
  trackCount: number;
  tracks: Track[];
}

export interface LyricLine {
  timeMs: number;
  text: string;
  translation: string | null;
}

// ==========================================
// 播放器模型 (Player Models)
// ==========================================

export type PlaybackState = 'stopped' | 'loading' | 'playing' | 'paused';
export type RepeatMode = 'off' | 'one' | 'all';
export type SoundQuality = 'standard' | 'higher' | 'exhigh' | 'lossless' | 'hires';

export interface PlaybackSnapshot {
  state: PlaybackState;
  positionMs: number;
  durationMs: number | null;
  volume: number; // 0.0 ~ 1.0
}

export interface PlayerSnapshot {
  sequence: string;
  queueRevision: string;
  selectionId: string;
  current: Track | null;
  currentIndex: number | null;
  queueLength: number;
  playback: PlaybackSnapshot;
  playWhenReady: boolean;
  resolving: boolean;
  repeat: RepeatMode;
  shuffle: boolean;
  canNext: boolean;
  canPrevious: boolean;
  canSeek: boolean;
  targetQuality: SoundQuality;
  actualQuality: SoundQuality | null;
  actualBitrate: number | null;
  format: string | null;
  bufferingPercent: number | null;
  isPreview: boolean;
  lastError: BackendError | null;
  seekSerial: string;
  seekPositionMs: number;
}

export interface PlayerQueue {
  revision: string;
  tracks: Track[];
  currentIndex: number | null;
}

// ==========================================
// 账号与会话模型 (Account & Session)
// ==========================================

export type QrStatus =
  | 'waiting_scan'
  | 'waiting_confirmation'
  | 'authenticated'
  | 'expired';

export type LoginPhase =
  | 'idle'
  | 'fetching'
  | 'waiting_scan'
  | 'waiting_confirmation'
  | 'authenticated'
  | 'expired'
  | 'failed';

export interface LoginError {
  code: string;
  message: string;
}

export type SessionPersistence =
  | 'none'
  | 'secure'
  | 'memory_only'
  | 'cleanup_required';

export interface UserProfile {
  id: string;
  nickname: string;
  avatarUrl: string | null;
}

export interface SessionSnapshot {
  profile: UserProfile | null;
  persistence: SessionPersistence;
}

export interface QrChallenge {
  attemptId: string;
  qrUrl: string;
  expiresInMs: number;
  pollIntervalMs: number;
}

export interface QrProgress {
  status: QrStatus;
  session: SessionSnapshot;
}

export interface LogoutReport {
  localCleared: boolean;
  credentialsCleared: boolean;
  serverRevoked: boolean;
}

export interface UserPlaylist {
  id: string;
  title: string;
  coverUrl: string | null;
  trackCount: number;
  ownerId: string;
  subscribed: boolean;
  isCreator: boolean;
  isLikedPlaylist: boolean;
}

export interface UserPlaylists {
  items: UserPlaylist[];
  offset: number;
  hasMore: boolean;
  likedPlaylistId: string | null;
}

// ==========================================
// 系统配置、状态与缓存 (System, Config & Cache)
// ==========================================

export type ProxyConfig =
  | { mode: 'system' }
  | { mode: 'direct' }
  | { mode: 'http'; url: string };

export interface AppConfig {
  version: number;
  cacheLimitBytes: number;
  proxy: ProxyConfig;
  restoreQueue: boolean;
  defaultQuality: SoundQuality;
}

export interface BackendStatus {
  mpris: 'starting' | 'ready' | 'unavailable' | string;
  persistence: 'ready' | 'degraded' | string;
  queueRestored: boolean;
  cacheAvailable: boolean;
}

export interface CacheStats {
  bytes: number;
  entries: number;
  limitBytes: number;
}

// ==========================================
// 全局事件负载 (Event Payloads)
// ==========================================

export interface SessionEventPayload {
  sequence: string;
  session: SessionSnapshot;
}

export type PlayerEventPayload = PlayerSnapshot;

export type BackendWarningPayload = 'mpris_unavailable' | string;

// ==========================================
// 错误模型 (Backend Error)
// ==========================================

export type BackendErrorCode =
  | 'storage'
  | 'already_running'
  | 'invalid_input'
  | 'network'
  | 'timeout'
  | 'http'
  | 'service'
  | 'protocol'
  | 'unavailable'
  | 'unauthorized'
  | 'stale_operation'
  | 'busy'
  | 'service_closed'
  | 'credential_storage'
  | 'audio';

export interface BackendError {
  code: BackendErrorCode;
  message?: string | number;
}
