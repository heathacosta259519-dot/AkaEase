import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import * as api from '../services/api';
import {
  sessionActions,
  getSessionState,
  parseLoginError,
  initSessionSubscription,
} from './sessionStore';
import type { QrChallenge, SessionEventPayload } from '../types/backend';

vi.mock('../services/api');

describe('sessionStore QR Login Lifecycle & Error Handling', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.clearAllMocks();
    sessionActions._resetForTesting();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('1. parseLoginError parses structured BackendError and generic errors without leaking secrets', () => {
    expect(parseLoginError({ code: 'busy' })).toEqual({
      code: 'busy',
      message: '登录请求进行中，请稍候',
    });
    expect(parseLoginError({ code: 'network' })).toEqual({
      code: 'network',
      message: '网络连接失败，请检查网络设置',
    });
    expect(parseLoginError({ code: 'timeout' })).toEqual({
      code: 'timeout',
      message: '网络连接超时，请重试',
    });
    expect(parseLoginError({ code: 'protocol' })).toEqual({
      code: 'protocol',
      message: '服务响应异常，请重试',
    });
    expect(parseLoginError(new Error('something bad'))).toEqual({
      code: 'unknown',
      message: 'something bad',
    });
  });

  it('2. Closes modal before login_begin resolves -> automatically cancels attempt on return, does not start poll', async () => {
    let resolveBegin!: (val: QrChallenge) => void;
    const beginPromise = new Promise<QrChallenge>((res) => {
      resolveBegin = res;
    });

    vi.mocked(api.loginBegin).mockReturnValue(beginPromise);
    vi.mocked(api.loginCancel).mockResolvedValue();

    sessionActions.openLoginModal();
    expect(getSessionState().loginPhase).toBe('fetching');
    expect(getSessionState().isLoginModalOpen).toBe(true);

    // User closes modal while loginBegin is in-flight
    sessionActions.closeLoginModal();
    expect(getSessionState().isLoginModalOpen).toBe(false);
    expect(getSessionState().loginPhase).toBe('idle');

    // loginBegin finally resolves late
    resolveBegin({
      attemptId: 'att-123',
      qrUrl: 'https://music.126.com/qr/123',
      expiresInMs: 180000,
      pollIntervalMs: 2000,
    });

    await Promise.resolve(); // drain microtasks

    // loginCancel should have been called for the late attempt
    expect(api.loginCancel).toHaveBeenCalledWith('att-123');
    // loginPoll should NOT have been called
    expect(api.loginPoll).not.toHaveBeenCalled();
    // Modal should remain closed and idle
    expect(getSessionState().isLoginModalOpen).toBe(false);
    expect(getSessionState().loginPhase).toBe('idle');
  });

  it('3. Rapid refresh clicks -> only latest generation succeeds and older attempt is cancelled', async () => {
    let resolveFirst!: (val: QrChallenge) => void;
    let resolveSecond!: (val: QrChallenge) => void;

    const firstPromise = new Promise<QrChallenge>((res) => {
      resolveFirst = res;
    });
    const secondPromise = new Promise<QrChallenge>((res) => {
      resolveSecond = res;
    });

    vi.mocked(api.loginBegin)
      .mockReturnValueOnce(firstPromise)
      .mockReturnValueOnce(secondPromise);
    vi.mocked(api.loginCancel).mockResolvedValue();

    sessionActions.openLoginModal(); // triggers first startLogin
    sessionActions.startLogin();     // immediately refresh

    // First resolves later
    resolveFirst({
      attemptId: 'att-first',
      qrUrl: 'https://music.126.com/qr/first',
      expiresInMs: 180000,
      pollIntervalMs: 2000,
    });
    await Promise.resolve();

    expect(api.loginCancel).toHaveBeenCalledWith('att-first');

    // Second resolves
    resolveSecond({
      attemptId: 'att-second',
      qrUrl: 'https://music.126.com/qr/second',
      expiresInMs: 180000,
      pollIntervalMs: 2000,
    });
    await Promise.resolve();

    expect(getSessionState().qrChallenge?.attemptId).toBe('att-second');
    expect(getSessionState().loginPhase).toBe('waiting_scan');
  });

  it('4. login_begin returns error -> ends fetching and sets failed phase with loginError', async () => {
    vi.mocked(api.loginBegin).mockRejectedValue({ code: 'busy' });

    sessionActions.openLoginModal();
    await Promise.resolve();

    expect(getSessionState().loginPhase).toBe('failed');
    expect(getSessionState().loginError?.code).toBe('busy');
    expect(api.loginPoll).not.toHaveBeenCalled();
  });

  it('5. Network errors during poll -> retries up to 3 times, fails on 4th consecutive error', async () => {
    vi.mocked(api.loginBegin).mockResolvedValue({
      attemptId: 'att-net',
      qrUrl: 'https://music.126.com/qr/net',
      expiresInMs: 180000,
      pollIntervalMs: 2000,
    });

    vi.mocked(api.loginPoll).mockRejectedValue({ code: 'network' });

    sessionActions.openLoginModal();
    await Promise.resolve();
    expect(getSessionState().loginPhase).toBe('waiting_scan');

    // 1st error retry
    await vi.advanceTimersByTimeAsync(2000);
    expect(getSessionState().loginPhase).toBe('waiting_scan');

    // 2nd error retry
    await vi.advanceTimersByTimeAsync(2000);
    expect(getSessionState().loginPhase).toBe('waiting_scan');

    // 3rd error retry
    await vi.advanceTimersByTimeAsync(2000);
    expect(getSessionState().loginPhase).toBe('waiting_scan');

    // 4th error -> terminal failure
    await vi.advanceTimersByTimeAsync(2000);
    expect(getSessionState().loginPhase).toBe('failed');
    expect(getSessionState().loginError?.code).toBe('network');
  });

  it('6. Busy error during poll -> retries without incrementing network error limit', async () => {
    vi.mocked(api.loginBegin).mockResolvedValue({
      attemptId: 'att-busy',
      qrUrl: 'https://music.126.com/qr/busy',
      expiresInMs: 180000,
      pollIntervalMs: 2000,
    });

    vi.mocked(api.loginPoll).mockRejectedValue({ code: 'busy' });

    sessionActions.openLoginModal();
    await Promise.resolve();

    // Advance 5 intervals with busy
    for (let i = 0; i < 5; i++) {
      await vi.advanceTimersByTimeAsync(2000);
      // Still in waiting_scan, not marked failed
      expect(getSessionState().loginPhase).toBe('waiting_scan');
    }
  });

  it('7. Terminal errors (protocol / unauthorized) -> terminates immediately', async () => {
    vi.mocked(api.loginBegin).mockResolvedValue({
      attemptId: 'att-proto',
      qrUrl: 'https://music.126.com/qr/proto',
      expiresInMs: 180000,
      pollIntervalMs: 2000,
    });

    vi.mocked(api.loginPoll).mockRejectedValue({ code: 'protocol' });

    sessionActions.openLoginModal();
    await Promise.resolve();

    await vi.advanceTimersByTimeAsync(2000);
    expect(getSessionState().loginPhase).toBe('failed');
    expect(getSessionState().loginError?.code).toBe('protocol');
  });

  it('8. Local deadline expiration -> stops polling and marks expired', async () => {
    vi.mocked(api.loginBegin).mockResolvedValue({
      attemptId: 'att-expire',
      qrUrl: 'https://music.126.com/qr/exp',
      expiresInMs: 4000, // short 4s lifetime
      pollIntervalMs: 2000,
    });

    vi.mocked(api.loginPoll).mockResolvedValue({
      status: 'waiting_scan',
      session: { profile: null, persistence: 'none' },
    });

    sessionActions.openLoginModal();
    await Promise.resolve();

    // 1st poll at 2s
    await vi.advanceTimersByTimeAsync(2000);
    expect(getSessionState().loginPhase).toBe('waiting_scan');

    // 2nd poll at 4s hits deadline
    await vi.advanceTimersByTimeAsync(2000);
    expect(getSessionState().loginPhase).toBe('expired');
  });

  it('9. Poll authenticated -> updates session immediately, closes modal, does not wait for secure', async () => {
    vi.mocked(api.loginBegin).mockResolvedValue({
      attemptId: 'att-auth',
      qrUrl: 'https://music.126.com/qr/auth',
      expiresInMs: 180000,
      pollIntervalMs: 2000,
    });

    vi.mocked(api.loginPoll).mockResolvedValue({
      status: 'authenticated',
      session: {
        profile: { id: 'u101', nickname: 'Alice', avatarUrl: null },
        persistence: 'memory_only', // Authenticated even when memory_only
      },
    });

    vi.mocked(api.getUserPlaylists).mockResolvedValue({
      items: [],
      offset: 0,
      hasMore: false,
      likedPlaylistId: null,
    });
    vi.mocked(api.getLikedTracks).mockResolvedValue([]);

    sessionActions.openLoginModal();
    await Promise.resolve();

    await vi.advanceTimersByTimeAsync(2000);

    expect(getSessionState().loginPhase).toBe('authenticated');
    expect(getSessionState().isLoginModalOpen).toBe(false);
    expect(getSessionState().session?.profile?.id).toBe('u101');
    expect(api.getUserPlaylists).toHaveBeenCalledTimes(1);
  });

  it('10. session-state with profile closes modal and avoids duplicate personal data fetch on persistence change', async () => {
    let sessionStateCallback!: (event: SessionEventPayload) => void;
    vi.mocked(api.onSessionState).mockImplementation((cb) => {
      sessionStateCallback = cb;
      return Promise.resolve(() => {});
    });
    vi.mocked(api.restoreSession).mockResolvedValue({
      profile: null,
      persistence: 'none',
    });
    vi.mocked(api.getUserPlaylists).mockResolvedValue({
      items: [],
      offset: 0,
      hasMore: false,
      likedPlaylistId: null,
    });
    vi.mocked(api.getLikedTracks).mockResolvedValue([]);

    await initSessionSubscription();

    sessionActions.openLoginModal();
    expect(getSessionState().isLoginModalOpen).toBe(true);

    // Event emitted with memory_only session
    sessionStateCallback({
      sequence: '1',
      session: {
        profile: { id: 'u202', nickname: 'Bob', avatarUrl: null },
        persistence: 'memory_only',
      },
    });

    expect(getSessionState().isLoginModalOpen).toBe(false);
    expect(getSessionState().session?.profile?.id).toBe('u202');
    expect(api.getUserPlaylists).toHaveBeenCalledTimes(1);

    // Later persistence changes to secure for the same user
    sessionStateCallback({
      sequence: '2',
      session: {
        profile: { id: 'u202', nickname: 'Bob', avatarUrl: null },
        persistence: 'secure',
      },
    });

    expect(getSessionState().session?.persistence).toBe('secure');
    // getUserPlaylists should NOT be called again!
    expect(api.getUserPlaylists).toHaveBeenCalledTimes(1);
  });

  it('11. user_playlists exposes canonical likedPlaylistId and isLikedPlaylist flag', async () => {
    let sessionStateCallback!: (event: SessionEventPayload) => void;
    vi.mocked(api.onSessionState).mockImplementation((cb) => {
      sessionStateCallback = cb;
      return Promise.resolve(() => {});
    });
    vi.mocked(api.restoreSession).mockResolvedValue({
      profile: null,
      persistence: 'none',
    });

    vi.mocked(api.getUserPlaylists).mockResolvedValue({
      items: [
        {
          id: 'liked-playlist-999',
          title: '我喜欢的音乐',
          coverUrl: null,
          trackCount: 42,
          ownerId: 'u303',
          subscribed: false,
          isCreator: true,
          isLikedPlaylist: true,
        },
        {
          id: 'custom-playlist-888',
          title: '流行精选',
          coverUrl: null,
          trackCount: 15,
          ownerId: 'u303',
          subscribed: false,
          isCreator: true,
          isLikedPlaylist: false,
        },
      ],
      offset: 0,
      hasMore: false,
      likedPlaylistId: 'liked-playlist-999',
    });
    vi.mocked(api.getLikedTracks).mockResolvedValue(['t1', 't2']);

    await initSessionSubscription();

    sessionStateCallback({
      sequence: '1',
      session: {
        profile: { id: 'u303', nickname: 'Charlie', avatarUrl: null },
        persistence: 'secure',
      },
    });

    // Wait for loadUserData promise to resolve
    await Promise.resolve();
    await Promise.resolve();

    const playlists = getSessionState().userPlaylists;
    expect(playlists?.likedPlaylistId).toBe('liked-playlist-999');
    expect(playlists?.items[0].isLikedPlaylist).toBe(true);
    expect(playlists?.items[1].isLikedPlaylist).toBe(false);

    // Filtered lists in sidebar:
    const createdOnly = playlists?.items.filter((p) => p.isCreator && !p.isLikedPlaylist);
    expect(createdOnly?.length).toBe(1);
    expect(createdOnly?.[0].id).toBe('custom-playlist-888');
  });

  describe('Library Management (Likes & Playlists)', () => {
    it('toggleLikeTrack prompts login when not logged in', async () => {
      const res = await sessionActions.toggleLikeTrack('song-1');
      expect(res).toBe(false);
      expect(getSessionState().isLoginModalOpen).toBe(true);
      expect(api.trackLike).not.toHaveBeenCalled();
    });

    it('toggleLikeTrack performs optimistic update and calls api.trackLike', async () => {
      // Simulate logged in
      (getSessionState() as any).session = {
        profile: { id: 'u101', nickname: 'Alice', avatarUrl: null },
        persistence: 'secure',
      };
      vi.mocked(api.trackLike).mockResolvedValue(true);

      const promise = sessionActions.toggleLikeTrack('song-100');
      // Optimistically added
      expect(getSessionState().likedIds.has('song-100')).toBe(true);
      await promise;
      expect(api.trackLike).toHaveBeenCalledWith('song-100', true);

      // Toggle off
      const promise2 = sessionActions.toggleLikeTrack('song-100');
      expect(getSessionState().likedIds.has('song-100')).toBe(false);
      await promise2;
      expect(api.trackLike).toHaveBeenCalledWith('song-100', false);
    });

    it('toggleLikeTrack rolls back optimistic update when api.trackLike fails', async () => {
      (getSessionState() as any).session = {
        profile: { id: 'u101', nickname: 'Alice', avatarUrl: null },
        persistence: 'secure',
      };
      vi.mocked(api.trackLike).mockRejectedValue(new Error('Network error'));

      await expect(sessionActions.toggleLikeTrack('song-err')).rejects.toThrow();
      expect(getSessionState().likedIds.has('song-err')).toBe(false);
    });

    it('createPlaylist inserts new playlist into userPlaylists', async () => {
      (getSessionState() as any).session = {
        profile: { id: 'u101', nickname: 'Alice', avatarUrl: null },
        persistence: 'secure',
      };
      (getSessionState() as any).userPlaylists = {
        items: [
          { id: 'p-liked', title: '我喜欢的音乐', coverUrl: null, trackCount: 5, ownerId: 'u101', subscribed: false, isCreator: true, isLikedPlaylist: true },
        ],
        offset: 0,
        hasMore: false,
        likedPlaylistId: 'p-liked',
      };

      vi.mocked(api.playlistCreate).mockResolvedValue({
        id: 'p-new',
        title: '我的新歌单',
        coverUrl: null,
        trackCount: 0,
        ownerId: 'u101',
        subscribed: false,
        isCreator: true,
        isLikedPlaylist: false,
      });

      const res = await sessionActions.createPlaylist('我的新歌单');
      expect(res.id).toBe('p-new');
      const items = getSessionState().userPlaylists?.items;
      expect(items?.length).toBe(2);
      expect(items?.[1].id).toBe('p-new');
    });

    it('deletePlaylist removes target playlist from userPlaylists', async () => {
      (getSessionState() as any).userPlaylists = {
        items: [
          { id: 'p-1', title: '歌单1', coverUrl: null, trackCount: 0, ownerId: 'u101', subscribed: false, isCreator: true, isLikedPlaylist: false },
          { id: 'p-2', title: '歌单2', coverUrl: null, trackCount: 0, ownerId: 'u101', subscribed: false, isCreator: true, isLikedPlaylist: false },
        ],
        offset: 0,
        hasMore: false,
        likedPlaylistId: null,
      };

      vi.mocked(api.playlistDelete).mockResolvedValue();
      await sessionActions.deletePlaylist('p-1');
      expect(api.playlistDelete).toHaveBeenCalledWith('p-1');
      expect(getSessionState().userPlaylists?.items.map((p) => p.id)).toEqual(['p-2']);
    });
  });
});
