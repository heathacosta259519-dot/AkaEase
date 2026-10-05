import { useSyncExternalStore } from 'react';

export function createStore<T extends object>(initialState: T) {
  let state = initialState;
  const listeners = new Set<() => void>();

  const getState = () => state;

  const setState = (updater: Partial<T> | ((prev: T) => Partial<T>)) => {
    const nextPartial = typeof updater === 'function' ? updater(state) : updater;
    state = { ...state, ...nextPartial };
    listeners.forEach((l) => l());
  };

  const subscribe = (listener: () => void) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  };

  const useStore = <U>(selector: (state: T) => U = (s) => s as unknown as U): U => {
    return useSyncExternalStore(
      subscribe,
      () => selector(state),
      () => selector(state),
    );
  };

  return { getState, setState, subscribe, useStore };
}
