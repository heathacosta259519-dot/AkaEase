import { ReactNode } from 'react';
import { Sidebar } from './Sidebar';
import { Header } from './Header';

interface AppLayoutProps {
  children: ReactNode;
  playerBar: ReactNode;
  queueDrawer: ReactNode;
  lyricsView: ReactNode;
  loginModal: ReactNode;
}

export function AppLayout({
  children,
  playerBar,
  queueDrawer,
  lyricsView,
  loginModal,
}: AppLayoutProps) {
  return (
    <div className="relative flex h-screen w-screen flex-col overflow-hidden bg-neutral-950 font-sans text-neutral-100 antialiased select-none">
      {/* Top Workspace Area */}
      <div className="flex flex-1 overflow-hidden">
        {/* Left Sidebar */}
        <Sidebar />

        {/* Center Main Workspace */}
        <div className="relative flex flex-1 flex-col overflow-hidden">
          <Header />
          <main className="gpu-scroll-container flex-1 overflow-y-auto bg-neutral-900/40 p-6">
            {children}
          </main>
        </div>
      </div>

      {/* Fixed Bottom Player Bar */}
      {playerBar}

      {/* Floating Drawers & Overlays */}
      {queueDrawer}
      {lyricsView}
      {loginModal}
    </div>
  );
}
