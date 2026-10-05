import { Play, Clock3 } from 'lucide-react';
import type { Track } from '../../types/backend';
import { usePlayerStore, playerActions } from '../../stores/playerStore';
import { formatDuration } from '../../utils/format';

interface SongTableProps {
  tracks: Track[];
  unavailableIds?: string[];
  showAlbum?: boolean;
  onPlayAll?: () => void;
}

export function SongTable({
  tracks,
  unavailableIds = [],
  showAlbum = true,
}: SongTableProps) {
  const snapshot = usePlayerStore((s) => s.snapshot);
  const currentTrack = snapshot?.current;
  const isPlaying = snapshot?.playback.state === 'playing';
  const unavailableSet = new Set(unavailableIds);

  const handlePlayRow = (index: number) => {
    playerActions.replaceQueue(tracks, index, true);
  };

  if (tracks.length === 0) {
    return (
      <div className="flex h-48 flex-col items-center justify-center text-xs text-neutral-500">
        暂无歌曲
      </div>
    );
  }

  return (
    <div className="w-full text-left text-xs select-none">
      {/* Table Header */}
      <div className="grid grid-cols-12 gap-3 border-b border-neutral-800/80 px-4 py-2.5 font-medium text-neutral-500 text-[11px] uppercase tracking-wider">
        <div className="col-span-1 text-center">#</div>
        <div className={showAlbum ? 'col-span-5' : 'col-span-7'}>歌曲标题</div>
        <div className="col-span-3">歌手</div>
        {showAlbum && <div className="col-span-2">专辑</div>}
        <div className="col-span-1 text-right flex justify-end items-center pr-2">
          <Clock3 className="h-3.5 w-3.5" />
        </div>
      </div>

      {/* Track Rows */}
      <div className="divide-y divide-neutral-900/40">
        {tracks.map((track, idx) => {
          const isCurrent = currentTrack?.id === track.id;
          const isUnavailable = unavailableSet.has(track.id);
          const artists = track.artists.map((a) => a.name).join(' / ');

          return (
            <div
              key={`${track.id}-${idx}`}
              onDoubleClick={() => !isUnavailable && handlePlayRow(idx)}
              className={`song-table-row group grid grid-cols-12 items-center gap-3 px-4 py-3 transition-colors rounded-xl ${
                isUnavailable
                  ? 'opacity-40 cursor-not-allowed'
                  : 'cursor-pointer hover:bg-neutral-800/60'
              } ${
                isCurrent
                  ? 'bg-neutral-800/50 text-rose-400 font-medium ring-1 ring-rose-500/20'
                  : 'text-neutral-300'
              }`}
            >
              {/* Index / Play action / Dynamic Equalizer */}
              <div className="col-span-1 flex items-center justify-center text-[12px] text-neutral-500 font-mono">
                {isCurrent ? (
                  isPlaying ? (
                    <div className="flex items-end gap-0.5 h-4 w-4 justify-center">
                      <span className="w-1 bg-rose-500 rounded-full eq-bar-1" />
                      <span className="w-1 bg-rose-500 rounded-full eq-bar-2" />
                      <span className="w-1 bg-rose-500 rounded-full eq-bar-3" />
                    </div>
                  ) : (
                    <div className="flex items-end gap-0.5 h-4 w-4 justify-center">
                      <span className="w-1 h-2 bg-rose-500/70 rounded-full" />
                      <span className="w-1 h-3.5 bg-rose-500/70 rounded-full" />
                      <span className="w-1 h-1.5 bg-rose-500/70 rounded-full" />
                    </div>
                  )
                ) : (
                  <>
                    <span className="group-hover:hidden">{idx + 1}</span>
                    <button
                      onClick={(e) => {
                        e.stopPropagation();
                        if (!isUnavailable) handlePlayRow(idx);
                      }}
                      className="hidden p-1 text-neutral-200 hover:text-white group-hover:inline-flex rounded hover:bg-neutral-700/60 transition-colors"
                      title="播放"
                    >
                      <Play className="h-3.5 w-3.5 fill-current ml-0.5" />
                    </button>
                  </>
                )}
              </div>

              {/* Title & Cover */}
              <div className={`col-span-${showAlbum ? '5' : '7'} flex items-center gap-3 min-w-0`}>
                <div className="relative h-10 w-10 rounded-lg overflow-hidden bg-neutral-800 shrink-0 shadow-sm border border-neutral-800 group-hover:border-neutral-700 transition-colors">
                  {track.album.coverUrl ? (
                    <img
                      src={track.album.coverUrl}
                      alt={track.title}
                      className="h-full w-full object-cover"
                      loading="lazy"
                    />
                  ) : (
                    <div className="h-full w-full flex items-center justify-center text-neutral-600 font-serif">
                      ♪
                    </div>
                  )}
                </div>

                <div className="flex flex-col min-w-0 pr-2">
                  <span
                    className={`truncate text-xs font-semibold ${
                      isCurrent
                        ? 'text-rose-400 font-bold'
                        : 'text-neutral-100 group-hover:text-white'
                    }`}
                  >
                    {track.title}
                  </span>
                  {isUnavailable && (
                    <span className="text-[10px] text-amber-500/80 font-medium">无版权 / VIP</span>
                  )}
                </div>
              </div>

              {/* Artists */}
              <div className="col-span-3 truncate text-neutral-400 group-hover:text-neutral-200 text-xs">
                {artists}
              </div>

              {/* Album */}
              {showAlbum && (
                <div className="col-span-2 truncate text-neutral-500 group-hover:text-neutral-400 text-xs">
                  {track.album.name}
                </div>
              )}

              {/* Duration */}
              <div className="col-span-1 text-right text-[11px] font-mono text-neutral-500 group-hover:text-neutral-300 pr-2">
                {formatDuration(track.durationMs)}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
