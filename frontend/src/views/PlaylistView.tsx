import { useState, useEffect } from 'react';
import { Play, Music2, RefreshCw, AlertCircle, Info, Sparkles, Clock } from 'lucide-react';
import { useViewStore } from '../stores/viewStore';
import { getPlaylist } from '../services/api';
import type { PlaylistPage } from '../types/backend';
import { SongTable } from '../components/music/SongTable';
import { playerActions } from '../stores/playerStore';

export function PlaylistView() {
  const activePlaylistId = useViewStore((s) => s.activePlaylistId);
  const [playlist, setPlaylist] = useState<PlaylistPage | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchPlaylist = () => {
    if (!activePlaylistId) return;
    setLoading(true);
    setError(null);

    getPlaylist(activePlaylistId, 0, 100)
      .then((data) => {
        setPlaylist(data);
      })
      .catch((err) => {
        console.error('Failed to load playlist:', err);
        setError('无法加载歌单，可能需要登录或网络异常');
      })
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    fetchPlaylist();
  }, [activePlaylistId]);

  const handlePlayAll = () => {
    if (playlist && playlist.tracks.items.length > 0) {
      playerActions.replaceQueue(playlist.tracks.items, 0, true);
    }
  };

  if (!activePlaylistId) {
    return (
      <div className="flex h-64 items-center justify-center text-xs text-neutral-500">
        请选择一个歌单
      </div>
    );
  }

  const coverUrl = playlist?.tracks.items[0]?.album.coverUrl;
  const totalDurationMs = playlist?.tracks.items.reduce((acc, t) => acc + t.durationMs, 0) ?? 0;

  return (
    <div className="space-y-6 max-w-6xl mx-auto select-none">
      {/* Immersive Playlist Header Hero Banner */}
      {playlist && (
        <div className="relative overflow-hidden rounded-2xl bg-neutral-900/60 border border-neutral-800/80 p-8 shadow-xl">
          {/* Subtle Dynamic Ambient Gradient Background */}
          {coverUrl && (
            <div
              className="absolute -right-16 -top-16 h-96 w-96 rounded-full pointer-events-none opacity-20"
              style={{
                background: 'radial-gradient(circle, rgba(225, 29, 72, 0.4) 0%, rgba(225, 29, 72, 0) 70%)',
              }}
            />
          )}

          <div className="relative z-10 flex flex-col sm:flex-row items-center sm:items-end gap-7">
            {/* High-Resolution Album Art Artwork */}
            <div className="relative h-44 w-44 overflow-hidden rounded-2xl bg-neutral-800 shadow-2xl ring-1 ring-white/10 shrink-0 group">
              {coverUrl ? (
                <img
                  src={coverUrl}
                  alt={playlist.title}
                  className="h-full w-full object-cover transition-transform duration-500 group-hover:scale-105"
                />
              ) : (
                <div className="flex h-full w-full items-center justify-center text-neutral-600">
                  <Music2 className="h-16 w-16" />
                </div>
              )}
            </div>

            {/* Playlist Meta Information Details */}
            <div className="flex flex-col gap-2.5 flex-1 min-w-0 text-center sm:text-left">
              <div className="flex items-center justify-center sm:justify-start gap-2">
                <span className="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full bg-rose-500/15 text-[10px] font-semibold text-rose-400 border border-rose-500/20 uppercase tracking-wider">
                  <Sparkles className="h-3 w-3" />
                  精选歌单
                </span>
                {playlist.unavailableIds.length > 0 && (
                  <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-neutral-800 text-[10px] text-neutral-400 border border-neutral-700/50">
                    <Info className="h-3 w-3 text-amber-500/80" />
                    {playlist.unavailableIds.length} 首不可用
                  </span>
                )}
              </div>

              <h1 className="text-2xl sm:text-3xl font-extrabold text-white tracking-tight truncate">
                {playlist.title}
              </h1>

              <div className="flex flex-wrap items-center justify-center sm:justify-start gap-4 text-xs text-neutral-400 font-medium pt-1">
                <span className="text-neutral-300">
                  共 <strong className="text-white font-semibold">{playlist.tracks.total}</strong> 首单曲
                </span>
                {totalDurationMs > 0 && (
                  <span className="flex items-center gap-1 text-neutral-400">
                    <Clock className="h-3.5 w-3.5 text-neutral-500" />
                    约 {Math.round(totalDurationMs / 60000)} 分钟
                  </span>
                )}
              </div>

              {/* Action Buttons Group */}
              <div className="pt-3 flex items-center justify-center sm:justify-start gap-3">
                <button
                  onClick={handlePlayAll}
                  disabled={playlist.tracks.items.length === 0}
                  className="inline-flex items-center gap-2 rounded-xl bg-gradient-to-r from-rose-600 to-rose-500 px-6 py-2.5 text-xs font-semibold text-white shadow-lg shadow-rose-950/60 hover:brightness-110 active:scale-95 disabled:opacity-40 transition-all cursor-pointer"
                >
                  <Play className="h-4 w-4 fill-current ml-0.5" />
                  <span>播放全部</span>
                </button>

                <button
                  onClick={fetchPlaylist}
                  disabled={loading}
                  title="刷新歌单"
                  className="inline-flex items-center gap-1.5 rounded-xl border border-neutral-800 bg-neutral-900/90 px-3.5 py-2.5 text-xs text-neutral-400 hover:text-white hover:bg-neutral-800 active:scale-95 transition-all cursor-pointer"
                >
                  <RefreshCw className={`h-3.5 w-3.5 ${loading ? 'animate-spin text-rose-500' : ''}`} />
                  <span>刷新</span>
                </button>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Content Area */}
      {loading ? (
        <div className="flex h-64 flex-col items-center justify-center text-xs text-neutral-500 gap-3">
          <RefreshCw className="h-5 w-5 animate-spin text-rose-500" />
          <span>正在加载歌单内容...</span>
        </div>
      ) : error ? (
        <div className="flex h-64 flex-col items-center justify-center text-xs text-neutral-400 gap-3">
          <AlertCircle className="h-7 w-7 text-amber-500" />
          <span>{error}</span>
          <button
            onClick={fetchPlaylist}
            className="mt-1 rounded-lg bg-neutral-800 px-3 py-1.5 text-xs text-rose-400 hover:text-rose-300 hover:bg-neutral-700 transition-colors"
          >
            点击重试
          </button>
        </div>
      ) : playlist ? (
        <SongTable
          tracks={playlist.tracks.items}
          unavailableIds={playlist.unavailableIds}
        />
      ) : null}
    </div>
  );
}
