import { useState, useEffect } from 'react';
import { X, Trash2, Music2, Volume2 } from 'lucide-react';
import { useViewStore, viewActions } from '../../stores/viewStore';
import { usePlayerStore, playerActions } from '../../stores/playerStore';
import { getPlayerQueue } from '../../services/api';
import type { PlayerQueue } from '../../types/backend';
import { formatDuration } from '../../utils/format';

export function QueueDrawer() {
  const isQueueOpen = useViewStore((s) => s.isQueueOpen);
  const snapshot = usePlayerStore((s) => s.snapshot);
  const queueRevision = snapshot?.queueRevision;
  const currentIndex = snapshot?.currentIndex;

  const [queue, setQueue] = useState<PlayerQueue | null>(null);

  useEffect(() => {
    if (isQueueOpen) {
      getPlayerQueue()
        .then(setQueue)
        .catch((err) => console.warn('Failed to load player queue:', err));
    }
  }, [isQueueOpen, queueRevision]);

  if (!isQueueOpen) return null;

  return (
    <div className="fixed inset-0 z-30 flex justify-end select-none">
      {/* Semi-transparent backdrop with click-to-dismiss */}
      <div
        className="fixed inset-0 bg-black/45 backdrop-blur-[2px] animate-backdrop-in transition-opacity"
        onClick={() => viewActions.setQueueOpen(false)}
      />

      {/* Drawer Panel */}
      <div className="relative z-10 flex h-full w-80 flex-col border-l border-neutral-800 bg-neutral-950/95 shadow-2xl backdrop-blur-md pb-[76px] animate-drawer-in">
        {/* Header */}
        <div className="flex h-14 items-center justify-between border-b border-neutral-800/80 px-4">
          <div className="flex items-center gap-2">
            <span className="font-semibold text-sm text-neutral-100">当前播放队列</span>
            <span className="text-xs text-neutral-500 font-mono">
              ({queue?.tracks.length ?? 0} 首)
            </span>
          </div>
          <button
            onClick={() => viewActions.setQueueOpen(false)}
            className="rounded p-1 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-100 press-feedback-sm"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        {/* Queue Track List */}
        <div className="flex-1 overflow-y-auto p-2 space-y-0.5 gpu-scroll-container">
          {!queue || queue.tracks.length === 0 ? (
            <div className="flex h-40 flex-col items-center justify-center text-xs text-neutral-500">
              <Music2 className="h-8 w-8 text-neutral-700 mb-2" />
              <span>队列为空</span>
            </div>
          ) : (
            queue.tracks.map((track, idx) => {
              const isPlaying = idx === currentIndex;
              const artists = track.artists.map((a) => a.name).join(', ');

              return (
                <div
                  key={`${track.id}-${idx}`}
                  className={`group flex items-center justify-between rounded-lg px-2.5 py-1.5 text-xs transition-all duration-150 cursor-pointer active:scale-[0.985] ${
                    isPlaying
                      ? 'bg-rose-500/15 text-rose-400 font-medium ring-1 ring-rose-500/20'
                      : 'text-neutral-300 hover:bg-neutral-900/80'
                  }`}
                  onClick={() => queue && playerActions.selectQueueItem(idx, queue.revision)}
                >
                  <div className="flex items-center gap-2.5 min-w-0 flex-1">
                    <div className="flex h-4 w-4 shrink-0 items-center justify-center text-[10px] text-neutral-500">
                      {isPlaying ? (
                        <Volume2 className="h-3.5 w-3.5 text-rose-500 animate-pulse" />
                      ) : (
                        idx + 1
                      )}
                    </div>

                    <div className="flex flex-col min-w-0">
                      <span className="truncate">{track.title}</span>
                      <span className="truncate text-[10px] text-neutral-500">
                        {artists}
                      </span>
                    </div>
                  </div>

                  <div className="flex items-center gap-2 shrink-0">
                    <span className="font-mono text-[10px] text-neutral-500">
                      {formatDuration(track.durationMs)}
                    </span>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        if (queue) playerActions.removeQueueItem(idx, queue.revision);
                      }}
                      title="从队列移除"
                      className="p-1 text-neutral-500 opacity-0 transition-opacity hover:text-rose-400 group-hover:opacity-100 press-feedback-sm"
                    >
                      <Trash2 className="h-3 w-3" />
                    </button>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>
    </div>
  );
}
