import { useEffect } from 'react';
import { useViewStore } from './stores/viewStore';
import { initPlayerSubscription, playerActions } from './stores/playerStore';
import { initSessionSubscription } from './stores/sessionStore';
import { initViewSubscription } from './stores/viewStore';

import { AppLayout } from './components/layout/AppLayout';
import { PlayerBar } from './components/player/PlayerBar';
import { QueueDrawer } from './components/player/QueueDrawer';
import { LyricsView } from './components/lyrics/LyricsView';
import { LoginModal } from './components/auth/LoginModal';

import { DiscoverView } from './views/DiscoverView';
import { DailyView } from './views/DailyView';
import { LikedView } from './views/LikedView';
import { PlaylistView } from './views/PlaylistView';
import { SearchView } from './views/SearchView';
import { SettingsView } from './views/SettingsView';

export default function App() {
  const currentView = useViewStore((s) => s.currentView);

  // Initialize all IPC subscriptions on application mount with strict cleanup guarding
  useEffect(() => {
    let isMounted = true;
    let unlistenPlayer: (() => void) | undefined;
    let unlistenSession: (() => void) | undefined;
    let unlistenWarning: (() => void) | undefined;

    initPlayerSubscription().then((fn) => {
      if (!isMounted) {
        fn();
      } else {
        unlistenPlayer = fn;
      }
    });

    initSessionSubscription().then((fn) => {
      if (!isMounted) {
        fn();
      } else {
        unlistenSession = fn;
      }
    });

    initViewSubscription().then((fn) => {
      if (!isMounted) {
        fn();
      } else {
        unlistenWarning = fn;
      }
    });

    // Global keyboard shortcut: Space to toggle play/pause
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.code === 'Space') {
        const target = e.target as HTMLElement;
        if (target.tagName !== 'INPUT' && target.tagName !== 'TEXTAREA') {
          e.preventDefault();
          playerActions.toggle();
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);

    return () => {
      isMounted = false;
      window.removeEventListener('keydown', handleKeyDown);
      unlistenPlayer?.();
      unlistenSession?.();
      unlistenWarning?.();
    };
  }, []);

  return (
    <AppLayout
      playerBar={<PlayerBar />}
      queueDrawer={<QueueDrawer />}
      lyricsView={<LyricsView />}
      loginModal={<LoginModal />}
    >
      {currentView === 'discover' && <DiscoverView />}
      {currentView === 'daily' && <DailyView />}
      {currentView === 'liked' && <LikedView />}
      {currentView === 'playlist' && <PlaylistView />}
      {currentView === 'search' && <SearchView />}
      {currentView === 'settings' && <SettingsView />}
    </AppLayout>
  );
}
