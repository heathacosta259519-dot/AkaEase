import { createStore } from './createStore';
import type {
  QrChallenge,
  SessionSnapshot,
  UserPlaylists,
} from '../types/backend';
import * as api from '../services/api';

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

interface SessionState {
  session: SessionSnapshot | null;
  userPlaylists: UserPlaylists | null;
  likedIds: Set<string>;
  isLoginModalOpen: boolean;
  qrChallenge: QrChallenge | null;
  loginPhase: LoginPhase;
  loginError: LoginError | null;
}

const store = createStore<SessionState>({
  session: null,
  userPlaylists: null,
  likedIds: new Set<string>(),
  isLoginModalOpen: false,
  qrChallenge: null,
  loginPhase: 'idle',
  loginError: null,
});

let loginGeneration = 0;
let pollTimer: ReturnType<typeof setTimeout> | null = null;
let lastLoadedProfileId: string | null = null;

export const useSessionStore = store.useStore;
export const getSessionState = store.getState;

export function parseLoginError(err: unknown): LoginError {
  if (typeof err === 'object' && err !== null && 'code' in err) {
    const code = String((err as { code: unknown }).code);
    const msgMap: Record<string, string> = {
      busy: '登录请求进行中，请稍候',
      network: '网络连接失败，请检查网络设置',
      timeout: '网络连接超时，请重试',
      protocol: '服务响应异常，请重试',
      unauthorized: '登录会话已失效',
      stale_operation: '操作已过期，请重试',
      storage: '本地存储异常',
      credential_storage: '系统密钥链不可用',
      image_error: '二维码生成失败，请重试',
    };
    return {
      code,
      message: msgMap[code] || '登录服务出现异常，请重试',
    };
  }
  if (err instanceof Error) {
    return { code: 'unknown', message: err.message || '登录异常，请重试' };
  }
  return { code: 'unknown', message: '未知错误，请重试' };
}

function clearPollTimer() {
  if (pollTimer) {
    clearTimeout(pollTimer);
    pollTimer = null;
  }
}

export async function initSessionSubscription() {
  // First subscribe to session-state events
  const unlisten = await api.onSessionState((event) => {
    store.setState({ session: event.session });

    // If an authenticated session is received, close any active login modal
    if (event.session.profile) {
      if (store.getState().isLoginModalOpen) {
        loginGeneration++;
        clearPollTimer();
        store.setState({
          isLoginModalOpen: false,
          loginPhase: 'authenticated',
          qrChallenge: null,
          loginError: null,
        });
      }
      triggerLoadUserData(event.session.profile.id);
    } else {
      triggerLoadUserData(null);
    }
  });

  // Then restore session on startup
  try {
    const initial = await api.restoreSession();
    store.setState({ session: initial });
    if (initial.profile) {
      triggerLoadUserData(initial.profile.id);
    }
  } catch (err) {
    console.warn('Session restore failed:', err);
  }

  return unlisten;
}

function triggerLoadUserData(profileId: string | null) {
  if (!profileId) {
    lastLoadedProfileId = null;
    store.setState({ userPlaylists: null, likedIds: new Set() });
    return;
  }
  // Avoid re-fetching personal data on pure persistence changes (e.g. memory_only -> secure)
  if (profileId === lastLoadedProfileId) {
    return;
  }
  lastLoadedProfileId = profileId;
  loadUserData();
}

async function loadUserData() {
  try {
    const [playlists, liked] = await Promise.allSettled([
      api.getUserPlaylists(0, 100),
      api.getLikedTracks(),
    ]);

    if (playlists.status === 'fulfilled') {
      store.setState({ userPlaylists: playlists.value });
    }
    if (liked.status === 'fulfilled') {
      store.setState({ likedIds: new Set(liked.value) });
    }
  } catch (err) {
    console.error('Failed to load user data:', err);
  }
}

function schedulePoll(
  attemptId: string,
  intervalMs: number,
  deadlineMs: number,
  generation: number,
  consecutiveErrors: number,
) {
  clearPollTimer();

  const current = store.getState();
  if (generation !== loginGeneration || !current.isLoginModalOpen) {
    return;
  }

  if (Date.now() >= deadlineMs) {
    store.setState({
      loginPhase: 'expired',
      qrChallenge: null,
    });
    return;
  }

  pollTimer = setTimeout(async () => {
    if (generation !== loginGeneration || !store.getState().isLoginModalOpen) {
      return;
    }

    if (Date.now() >= deadlineMs) {
      store.setState({
        loginPhase: 'expired',
        qrChallenge: null,
      });
      return;
    }

    try {
      const progress = await api.loginPoll(attemptId);

      if (generation !== loginGeneration || !store.getState().isLoginModalOpen) {
        return;
      }

      if (progress.status === 'authenticated') {
        store.setState({
          session: progress.session,
          loginPhase: 'authenticated',
          isLoginModalOpen: false,
          qrChallenge: null,
          loginError: null,
        });
        triggerLoadUserData(progress.session.profile?.id ?? null);
        return;
      }

      if (progress.status === 'expired') {
        store.setState({
          loginPhase: 'expired',
          qrChallenge: null,
        });
        return;
      }

      const nextPhase: LoginPhase =
        progress.status === 'waiting_confirmation'
          ? 'waiting_confirmation'
          : 'waiting_scan';

      store.setState({
        loginPhase: nextPhase,
        loginError: null,
      });

      schedulePoll(attemptId, intervalMs, deadlineMs, generation, 0);
    } catch (err) {
      if (generation !== loginGeneration || !store.getState().isLoginModalOpen) {
        return;
      }

      const parsed = parseLoginError(err);

      // Busy: backend is polling too frequently or checking in progress, wait and retry without increasing consecutive error count
      if (parsed.code === 'busy') {
        schedulePoll(attemptId, intervalMs, deadlineMs, generation, consecutiveErrors);
        return;
      }

      // Transient network or server 5xx errors: limited retry up to 3 times
      if (
        parsed.code === 'network' ||
        parsed.code === 'timeout' ||
        parsed.code.startsWith('http') ||
        parsed.code === 'service'
      ) {
        const nextErrors = consecutiveErrors + 1;
        if (nextErrors <= 3) {
          schedulePoll(attemptId, intervalMs, deadlineMs, generation, nextErrors);
          return;
        }
      }

      // Terminal error: protocol mismatch, unauthorized, stale attempt or exceeded retries
      store.setState({
        loginPhase: 'failed',
        loginError: parsed,
        qrChallenge: null,
      });
    }
  }, intervalMs);
}

export const sessionActions = {
  openLoginModal() {
    store.setState({
      isLoginModalOpen: true,
      loginPhase: 'idle',
      loginError: null,
      qrChallenge: null,
    });
    sessionActions.startLogin();
  },

  closeLoginModal() {
    const current = store.getState();
    loginGeneration++;
    clearPollTimer();

    if (current.qrChallenge) {
      api.loginCancel(current.qrChallenge.attemptId).catch(() => {});
    }

    store.setState({
      isLoginModalOpen: false,
      loginPhase: 'idle',
      loginError: null,
      qrChallenge: null,
    });
  },

  async startLogin() {
    clearPollTimer();

    const current = store.getState();
    if (current.qrChallenge) {
      api.loginCancel(current.qrChallenge.attemptId).catch(() => {});
    }

    const generation = ++loginGeneration;

    store.setState({
      loginPhase: 'fetching',
      loginError: null,
      qrChallenge: null,
    });

    try {
      const challenge = await api.loginBegin();

      // Check if modal closed or another login was triggered while loginBegin was awaiting
      const latest = store.getState();
      if (generation !== loginGeneration || !latest.isLoginModalOpen) {
        api.loginCancel(challenge.attemptId).catch(() => {});
        return;
      }

      const deadlineMs = Date.now() + challenge.expiresInMs;

      store.setState({
        qrChallenge: challenge,
        loginPhase: 'waiting_scan',
        loginError: null,
      });

      schedulePoll(
        challenge.attemptId,
        challenge.pollIntervalMs || 2000,
        deadlineMs,
        generation,
        0,
      );
    } catch (err) {
      const latest = store.getState();
      if (generation !== loginGeneration || !latest.isLoginModalOpen) {
        return;
      }

      store.setState({
        loginPhase: 'failed',
        loginError: parseLoginError(err),
        qrChallenge: null,
      });
    }
  },

  setImageError(err: unknown) {
    store.setState({
      loginPhase: 'failed',
      loginError: parseLoginError(err || { code: 'image_error' }),
    });
  },

  async logout() {
    loginGeneration++;
    clearPollTimer();
    lastLoadedProfileId = null;

    try {
      const report = await api.logout();
      const latest = await api.getSessionSnapshot();
      store.setState({
        session: latest,
        userPlaylists: null,
        likedIds: new Set(),
        loginPhase: 'idle',
        loginError: null,
        qrChallenge: null,
      });
      return report;
    } catch (err) {
      console.error('Logout error:', err);
    }
  },

  async refresh() {
    try {
      const snap = await api.refreshSession();
      store.setState({ session: snap });
      if (snap.profile) {
        triggerLoadUserData(snap.profile.id);
      }
    } catch (err) {
      console.error('Session refresh error:', err);
    }
  },

  // Test helpers to reset internal generation state
  _resetForTesting() {
    loginGeneration = 0;
    clearPollTimer();
    lastLoadedProfileId = null;
    store.setState({
      session: null,
      userPlaylists: null,
      likedIds: new Set(),
      isLoginModalOpen: false,
      qrChallenge: null,
      loginPhase: 'idle',
      loginError: null,
    });
  },
};
