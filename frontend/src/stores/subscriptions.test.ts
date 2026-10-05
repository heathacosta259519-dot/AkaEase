import { describe, it, expect, vi } from 'vitest';
import * as api from '../services/api';
import { initSessionSubscription } from './sessionStore';

vi.mock('../services/api');

describe('Subscription strict unmount guarding', () => {
  it('Guards against unmount before async subscription resolves (StrictMode resilience)', async () => {
    let unlistenCalled = false;
    const fakeUnlisten = () => {
      unlistenCalled = true;
    };

    let resolveOnSessionState!: (fn: () => void) => void;
    const onSessionStatePromise = new Promise<() => void>((res) => {
      resolveOnSessionState = res;
    });

    vi.mocked(api.onSessionState).mockImplementation(() => onSessionStatePromise);
    vi.mocked(api.restoreSession).mockResolvedValue({
      profile: null,
      persistence: 'none',
    });

    // Simulate App mounting and unmounting immediately before promise resolves
    let isMounted = true;
    let unlistenSession: (() => void) | undefined;

    initSessionSubscription().then((fn) => {
      if (!isMounted) {
        fn();
      } else {
        unlistenSession = fn;
      }
    });

    // Immediate unmount
    isMounted = false;
    unlistenSession?.();

    // Now async subscription finishes resolving
    resolveOnSessionState(fakeUnlisten);
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();

    // The unlisten function must have been called upon late resolution!
    expect(unlistenCalled).toBe(true);
  });
});
