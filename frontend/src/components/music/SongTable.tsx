import { useState, useRef, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { Play, Clock3, Heart, MoreHorizontal, Plus, Trash2, Check } from 'lucide-react';
import type { Track } from '../../types/backend';
import { usePlayerStore, playerActions } from '../../stores/playerStore';
import { viewActions } from '../../stores/viewStore';
import { useSessionStore, sessionActions } from '../../stores/sessionStore';
import { formatDuration } from '../../utils/format';

interface SongTableProps {
  tracks: Track[];
  unavailableIds?: string[];
  showAlbum?: boolean;
  onPlayAll?: () => void;
  onPlayTrack?: (track: Track) => void;
  playlistId?: string;
  onRemoveFromPlaylist?: (track: Track) => void;
}

export function SongTable({
  tracks,
  unavailableIds = [],
  showAlbum = true,
  onPlayTrack,
  playlistId,
  onRemoveFromPlaylist,
}: SongTableProps) {
  const snapshot = usePlayerStore((s) => s.snapshot);
  const currentTrack = snapshot?.current;
  const isPlaying = snapshot?.playback.state === 'playing';
  const unavailableSet = new Set(unavailableIds);

  const likedIds = useSessionStore((s) => s.likedIds);
  const userPlaylists = useSessionStore((s) => s.userPlaylists);
  const createdPlaylists = userPlaylists?.items.filter((p) => p.isCreator && !p.isLikedPlaylist) ?? [];
  const targetPlaylists = playlistId
    ? createdPlaylists.filter((p) => p.id !== playlistId)
    : createdPlaylists;

  // Dropdown menu state via top-level Portal (bypasses all contain:paint / content-visibility / overflow clipping)
  const [menuAnchor, setMenuAnchor] = useState<{
    track: Track;
    rect: DOMRect;
    direction: 'down' | 'up';
  } | null>(null);

  const [addedFeedback, setAddedFeedback] = useState<string | null>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClose = (e?: Event) => {
      // If clicking inside menu, do not close immediately unless specified
      if (e && e.type === 'mousedown' && menuRef.current?.contains((e as MouseEvent).target as Node)) {
        return;
      }
      setMenuAnchor(null);
      setAddedFeedback(null);
    };

    if (menuAnchor) {
      document.addEventListener('mousedown', handleClose);
      window.addEventListener('scroll', handleClose, true);
      const handleKeyDown = (e: KeyboardEvent) => {
        if (e.key === 'Escape') handleClose();
      };
      window.addEventListener('keydown', handleKeyDown);

      return () => {
        document.removeEventListener('mousedown', handleClose);
        window.removeEventListener('scroll', handleClose, true);
        window.removeEventListener('keydown', handleKeyDown);
      };
    }
  }, [menuAnchor]);

  const handlePlayRow = (index: number) => {
    if (onPlayTrack) {
      onPlayTrack(tracks[index]);
    } else {
      playerActions.replaceQueue(tracks, index, true);
    }
  };

  const handleOpenMenu = (e: React.MouseEvent, track: Track) => {
    e.stopPropagation();
    if (menuAnchor?.track.id === track.id) {
      setMenuAnchor(null);
      return;
    }
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const spaceBelow = window.innerHeight - rect.bottom;
    setMenuAnchor({
      track,
      rect,
      direction: spaceBelow < 260 ? 'up' : 'down',
    });
  };

  const handleAddToPlaylist = async (targetPlaylistId: string, track: Track) => {
    try {
      await sessionActions.addTracksToPlaylist(targetPlaylistId, [track.id]);
      setAddedFeedback(targetPlaylistId);
      setTimeout(() => {
        setMenuAnchor(null);
        setAddedFeedback(null);
      }, 500);
    } catch (err) {
      console.error('Failed to add track to playlist:', err);
    }
  };

  if (tracks.length === 0) {
    return (
      <div className="flex h-48 flex-col items-center justify-center text-xs text-neutral-500">
        暂无歌曲
      </div>
    );
  }

  return (
    <div className="w-full text-left text-sm select-none pb-24">
      {/* Table Header */}
      <div className="grid grid-cols-12 gap-4 border-b border-neutral-800/80 px-4 py-3 font-semibold text-neutral-400 text-xs tracking-wider">
        <div className="col-span-1 text-center flex items-center justify-center">
          <span>#</span>
        </div>
        <div className={showAlbum ? 'col-span-4' : 'col-span-6'}>歌曲标题</div>
        <div className="col-span-3">歌手</div>
        {showAlbum && <div className="col-span-2">专辑</div>}
        <div className={`${showAlbum ? 'col-span-2' : 'col-span-2'} text-right flex justify-end items-center pr-2`}>
          <Clock3 className="h-4 w-4" />
        </div>
      </div>

      {/* Track Rows */}
      <div className="divide-y divide-neutral-900/40">
        {tracks.map((track, idx) => {
          const isCurrent = currentTrack?.id === track.id;
          const isUnavailable = unavailableSet.has(track.id);
          const isLiked = likedIds.has(track.id);
          const isRowActive = menuAnchor?.track.id === track.id;

          return (
            <div
              key={`${track.id}-${idx}`}
              onDoubleClick={() => !isUnavailable && handlePlayRow(idx)}
              className={`song-table-row group grid grid-cols-12 items-center gap-4 px-4 py-3.5 transition-all duration-150 rounded-xl relative ${
                isUnavailable
                  ? 'opacity-40 cursor-not-allowed'
                  : 'cursor-pointer hover:bg-neutral-800/60 active:scale-[0.995]'
              } ${
                isCurrent
                  ? 'bg-neutral-800/50 text-rose-400 font-medium ring-1 ring-rose-500/20'
                  : 'text-neutral-300'
              } ${isRowActive ? '!bg-neutral-800/80 ring-1 ring-neutral-700' : ''}`}
            >
              {/* Index / Play / Heart action */}
              <div className="col-span-1 flex items-center justify-center gap-1.5 text-xs text-neutral-400 font-mono">
                <div className="w-5 flex items-center justify-center">
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
                        className="hidden p-1 text-neutral-200 hover:text-white group-hover:inline-flex rounded hover:bg-neutral-700/60 transition-colors press-feedback-sm cursor-pointer"
                        title="播放"
                      >
                        <Play className="h-3.5 w-3.5 fill-current ml-0.5" />
                      </button>
                    </>
                  )}
                </div>

                {/* Heart Button */}
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    sessionActions.toggleLikeTrack(track.id);
                  }}
                  title={isLiked ? '取消喜欢' : '喜欢'}
                  className={`p-1 rounded transition-all press-feedback-sm cursor-pointer ${
                    isLiked
                      ? 'text-rose-500'
                      : 'text-neutral-500 hover:text-neutral-300 opacity-0 group-hover:opacity-100'
                  }`}
                >
                  <Heart className={`h-3.5 w-3.5 ${isLiked ? 'fill-rose-500 text-rose-500' : ''}`} />
                </button>
              </div>

              {/* Title & Cover */}
              <div className={`${showAlbum ? 'col-span-4' : 'col-span-6'} flex items-center gap-3.5 min-w-0`}>
                <div className="relative h-11 w-11 rounded-xl overflow-hidden bg-neutral-800 shrink-0 shadow-md border border-neutral-800 group-hover:border-neutral-700 transition-colors">
                  {track.album.coverUrl ? (
                    <img
                      src={track.album.coverUrl}
                      alt={track.title}
                      className="h-full w-full object-cover transition-transform duration-300 group-hover:scale-105"
                      loading="lazy"
                    />
                  ) : (
                    <div className="h-full w-full flex items-center justify-center text-neutral-600 font-serif text-base">
                      ♪
                    </div>
                  )}
                </div>

                <div className="flex flex-col min-w-0 pr-2 gap-0.5">
                  <span
                    className={`truncate text-sm font-semibold ${
                      isCurrent
                        ? 'text-rose-400 font-bold'
                        : 'text-neutral-100 group-hover:text-white'
                    }`}
                  >
                    {track.title}
                  </span>
                  {isUnavailable && (
                    <span className="text-[11px] text-amber-500/90 font-medium">无版权 / VIP</span>
                  )}
                </div>
              </div>

              {/* Artists */}
              <div className="col-span-3 truncate text-neutral-300 text-[13px] pr-2">
                {track.artists.map((artist, aIdx) => (
                  <span key={artist.id || aIdx} className="inline">
                    <span
                      onClick={(e) => {
                        e.stopPropagation();
                        viewActions.openArtist(artist.id);
                      }}
                      className="cursor-pointer hover:text-white hover:underline transition-colors"
                    >
                      {artist.name}
                    </span>
                    {aIdx < track.artists.length - 1 && <span className="text-neutral-500 mr-1">, </span>}
                  </span>
                ))}
              </div>

              {/* Album */}
              {showAlbum && (
                <div className="col-span-2 truncate text-neutral-400 text-[13px] pr-2">
                  {track.album.id ? (
                    <span
                      onClick={(e) => {
                        e.stopPropagation();
                        viewActions.openAlbum(track.album.id);
                      }}
                      className="cursor-pointer hover:text-neutral-200 hover:underline transition-colors"
                    >
                      {track.album.name}
                    </span>
                  ) : (
                    <span>{track.album.name}</span>
                  )}
                </div>
              )}

              {/* Duration & More Options */}
              <div className={`${showAlbum ? 'col-span-2' : 'col-span-2'} flex items-center justify-end gap-2.5 text-xs font-mono text-neutral-400 group-hover:text-neutral-200 pr-2`}>
                <span className="shrink-0">{formatDuration(track.durationMs)}</span>
                
                <div className="relative shrink-0">
                  <button
                    onClick={(e) => handleOpenMenu(e, track)}
                    title="更多选项"
                    className="p-1 rounded text-neutral-400 hover:text-white hover:bg-neutral-700/60 opacity-0 group-hover:opacity-100 transition-all cursor-pointer"
                  >
                    <MoreHorizontal className="h-4 w-4" />
                  </button>
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {/* Top-Level Portal Floating Dropdown Menu (immune to contain:paint, overflow-hidden, and stacking-context bugs) */}
      {menuAnchor &&
        createPortal(
          <div
            ref={menuRef}
            onClick={(e) => e.stopPropagation()}
            style={{
              position: 'fixed',
              right: `${Math.max(16, window.innerWidth - menuAnchor.rect.right)}px`,
              ...(menuAnchor.direction === 'up'
                ? { bottom: `${window.innerHeight - menuAnchor.rect.top + 6}px` }
                : { top: `${menuAnchor.rect.bottom + 6}px` }),
              zIndex: 9999,
            }}
            className={`w-56 rounded-2xl border border-neutral-700/90 bg-neutral-900/98 p-1.5 shadow-2xl backdrop-blur-2xl text-xs text-neutral-200 animate-in fade-in zoom-in-95 duration-100 ${
              menuAnchor.direction === 'up' ? 'origin-bottom-right' : 'origin-top-right'
            }`}
          >
            {/* Add to Playlist Subheader */}
            <div className="px-3 py-1 font-semibold text-neutral-400 text-[11px] uppercase tracking-wider flex items-center gap-1.5">
              <Plus className="h-3.5 w-3.5 text-rose-500" />
              <span>添加到歌单</span>
            </div>

            {targetPlaylists.length === 0 ? (
              <div className="px-3 py-2 text-neutral-500 text-[11px]">
                暂无其他自建歌单
              </div>
            ) : (
              <div className="max-h-52 overflow-y-auto py-0.5 space-y-0.5">
                {targetPlaylists.map((pl) => {
                  const isAdded = addedFeedback === pl.id;
                  return (
                    <button
                      key={pl.id}
                      onClick={() => handleAddToPlaylist(pl.id, menuAnchor.track)}
                      className="flex w-full items-center justify-between px-3 py-1.5 rounded-lg hover:bg-white/10 transition-colors truncate cursor-pointer text-left"
                    >
                      <span className="truncate pr-2 text-neutral-200 hover:text-white font-medium">
                        {pl.title}
                      </span>
                      {isAdded && <Check className="h-3.5 w-3.5 text-emerald-400 shrink-0" />}
                    </button>
                  );
                })}
              </div>
            )}

            {/* Remove from current playlist if provided */}
            {onRemoveFromPlaylist && (
              <>
                <div className="my-1 border-t border-neutral-800" />
                <button
                  onClick={() => {
                    const tr = menuAnchor.track;
                    setMenuAnchor(null);
                    onRemoveFromPlaylist(tr);
                  }}
                  className="flex w-full items-center gap-2 px-3 py-1.5 rounded-lg text-left text-rose-400 hover:bg-rose-500/15 transition-colors cursor-pointer"
                >
                  <Trash2 className="h-3.5 w-3.5" />
                  <span>从当前歌单移除</span>
                </button>
              </>
            )}
          </div>,
          document.body
        )}
    </div>
  );
}
