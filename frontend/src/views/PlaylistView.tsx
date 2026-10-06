import { useState, useEffect, useRef, useCallback } from 'react';
import { Play, Music2, RefreshCw, AlertCircle, Info, Sparkles, Clock, Loader2 } from 'lucide-react';
import { useViewStore } from '../stores/viewStore';
import { getPlaylist } from '../services/api';
import type { PlaylistPage } from '../types/backend';
import { SongTable } from '../components/music/SongTable';
import { playerActions, usePlayerStore } from '../stores/playerStore';

export function PlaylistView() {
  const activePlaylistId = useViewStore((s) => s.activePlaylistId);
  const queueLoading = usePlayerStore((s) => s.loadingPlaylistId === activePlaylistId && activePlaylistId !== null);
  const queueError = usePlayerStore((s) => s.playlistError?.id === activePlaylistId ? s.playlistError.message : null);
  const [playlist, setPlaylist] = useState<PlaylistPage | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [loadingMore, setLoadingMore] = useState(false);
  const [loadMoreError, setLoadMoreError] = useState<string | null>(null);

  const requestGenRef = useRef(0);
  const loadingMoreRef = useRef(false);
  const sentinelRef = useRef<HTMLDivElement>(null);

  const fetchPlaylist = () => {
    if (!activePlaylistId) return;
    setLoading(true);
    setError(null);
    setLoadingMore(false);
    setLoadMoreError(null);
    const currentGen = ++requestGenRef.current;

    getPlaylist(activePlaylistId, 0, 100)
      .then((data) => {
        if (currentGen === requestGenRef.current) {
          setPlaylist(data);
        }
      })
      .catch((err) => {
        if (currentGen === requestGenRef.current) {
          console.error('Failed to load playlist:', err);
          setError('无法加载歌单，可能需要登录或网络异常');
        }
      })
      .finally(() => {
        if (currentGen === requestGenRef.current) {
          setLoading(false);
        }
      });
  };

  useEffect(() => {
    fetchPlaylist();
  }, [activePlaylistId]);

  const loadNextPage = useCallback(async () => {
    if (!activePlaylistId || !playlist || loading || loadingMoreRef.current) return;
    if (!playlist.tracks.hasMore) return;

    const offset = playlist.tracks.items.length;
    if (offset >= playlist.tracks.total) return;

    loadingMoreRef.current = true;
    setLoadingMore(true);
    setLoadMoreError(null);
    const currentGen = requestGenRef.current;

    try {
      const nextPage = await getPlaylist(activePlaylistId, offset, 100);
      if (currentGen !== requestGenRef.current) return;

      setPlaylist((prev) => {
        if (!prev || prev.id !== activePlaylistId) return prev;
        const existingIds = new Set(prev.tracks.items.map((t) => t.id));
        const incoming = nextPage.tracks.items.filter((t) => !existingIds.has(t.id));
        const mergedItems = [...prev.tracks.items, ...incoming];
        const unavailableSet = new Set([...prev.unavailableIds, ...nextPage.unavailableIds]);

        return {
          ...prev,
          tracks: {
            ...nextPage.tracks,
            items: mergedItems,
            offset,
            hasMore:
              nextPage.tracks.hasMore &&
              incoming.length > 0 &&
              mergedItems.length < prev.tracks.total,
          },
          unavailableIds: Array.from(unavailableSet),
        };
      });
    } catch (err) {
      if (currentGen !== requestGenRef.current) return;
      console.error('Failed to load more tracks:', err);
      setLoadMoreError('加载后续歌曲失败，请点击重试');
    } finally {
      if (currentGen === requestGenRef.current) {
        loadingMoreRef.current = false;
        setLoadingMore(false);
      }
    }
  }, [activePlaylistId, playlist, loading]);

  // Infinite scroll trigger via IntersectionObserver with 300px lookahead
  useEffect(() => {
    const sentinel = sentinelRef.current;
    if (!sentinel || !playlist?.tracks.hasMore) return;

    const observer = new IntersectionObserver(
      (entries) => {
        const first = entries[0];
        if (first && first.isIntersecting && !loadingMoreRef.current) {
          loadNextPage();
        }
      },
      {
        rootMargin: '300px',
      }
    );

    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [playlist?.tracks.hasMore, loadNextPage]);

  const handlePlayAll = () => {
    if (playlist && playlist.tracks.items.length > 0) {
      playerActions.replacePlaylistQueue(playlist.id, playlist.tracks.items);
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
            <div className="relative h-48 w-48 overflow-hidden rounded-2xl bg-neutral-800 shadow-2xl ring-1 ring-white/10 shrink-0 group">
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
              <div className="flex items-center justify-center sm:justify-start gap-2.5">
                <span className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-rose-500/15 text-xs font-semibold text-rose-400 border border-rose-500/20 uppercase tracking-wider">
                  <Sparkles className="h-3.5 w-3.5" />
                  精选歌单
                </span>
                {playlist.unavailableIds.length > 0 && (
                  <span className="inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-neutral-800 text-xs text-neutral-400 border border-neutral-700/50">
                    <Info className="h-3.5 w-3.5 text-amber-500/80" />
                    {playlist.unavailableIds.length} 首不可用
                  </span>
                )}
              </div>

              <h1 className="text-3xl sm:text-4xl font-extrabold text-white tracking-tight truncate leading-tight">
                {playlist.title}
              </h1>

              <div className="flex flex-wrap items-center justify-center sm:justify-start gap-5 text-sm text-neutral-300 font-medium pt-1">
                <span>
                  {playlist.tracks.items.length < playlist.tracks.total ? (
                    <>
                      已加载 <strong className="text-rose-400 font-semibold">{playlist.tracks.items.length}</strong> / 共{' '}
                      <strong className="text-white font-bold">{playlist.tracks.total}</strong> 首单曲
                    </>
                  ) : (
                    <>
                      共 <strong className="text-white font-bold">{playlist.tracks.total}</strong> 首单曲
                    </>
                  )}
                </span>
                {totalDurationMs > 0 && (
                  <span className="flex items-center gap-1.5 text-neutral-400">
                    <Clock className="h-4 w-4 text-neutral-500" />
                    约 {Math.round(totalDurationMs / 60000)} 分钟
                  </span>
                )}
              </div>

              {/* Action Buttons Group */}
              <div className="pt-2 flex items-center justify-center sm:justify-start gap-3.5">
                <button
                  onClick={handlePlayAll}
                  disabled={playlist.tracks.items.length === 0 || queueLoading}
                  className="inline-flex items-center gap-2.5 rounded-xl bg-gradient-to-r from-rose-600 to-rose-500 px-7 py-2.5 text-sm font-semibold text-white shadow-lg shadow-rose-950/60 hover:brightness-110 active:scale-95 disabled:opacity-40 transition-all cursor-pointer press-feedback"
                >
                  {queueLoading ? <Loader2 className="h-4.5 w-4.5 animate-spin" /> : <Play className="h-4.5 w-4.5 fill-current ml-0.5" />}
                  <span>播放全部</span>
                </button>

                <button
                  onClick={fetchPlaylist}
                  disabled={loading}
                  title="刷新歌单"
                  className="inline-flex items-center gap-2 rounded-xl border border-neutral-800 bg-neutral-900/90 px-4 py-2.5 text-sm text-neutral-300 hover:text-white hover:bg-neutral-800 active:scale-95 transition-all cursor-pointer press-feedback-sm"
                >
                  <RefreshCw className={`h-4 w-4 ${loading ? 'animate-spin text-rose-500' : ''}`} />
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
        <div className="space-y-4">
          <SongTable
            tracks={playlist.tracks.items}
            unavailableIds={playlist.unavailableIds}
            onPlayTrack={(track) => { playerActions.replacePlaylistQueue(playlist.id, playlist.tracks.items, track.id); }}
          />
          {queueLoading && <p role="status" className="text-xs text-neutral-400">正在后台补齐歌单...</p>}
          {queueError && <p role="alert" className="text-xs text-rose-400">{queueError}</p>}

          {/* Infinite Scroll Sentinel & Loading Indicator */}
          <div
            ref={sentinelRef}
            className="py-6 flex flex-col items-center justify-center text-xs text-neutral-500"
          >
            {loadingMore && (
              <div className="flex items-center gap-2 text-rose-400 font-medium py-2">
                <Loader2 className="h-4 w-4 animate-spin text-rose-500" />
                <span>
                  正在加载更多歌曲 ({playlist.tracks.items.length} / {playlist.tracks.total})...
                </span>
              </div>
            )}

            {loadMoreError && (
              <div className="flex flex-col items-center gap-2 text-neutral-400 py-2">
                <span className="text-rose-400">{loadMoreError}</span>
                <button
                  onClick={loadNextPage}
                  className="rounded-lg bg-neutral-800 px-3 py-1.5 text-xs text-rose-400 hover:text-rose-300 hover:bg-neutral-700 transition-colors press-feedback-sm"
                >
                  点击重试
                </button>
              </div>
            )}

            {!playlist.tracks.hasMore && playlist.tracks.items.length > 50 && (
              <div className="text-neutral-600 text-[11px] py-2">
                —— 已加载全部 {playlist.tracks.items.length} 首歌曲 ——
              </div>
            )}

            {playlist.tracks.hasMore && !loadingMore && !loadMoreError && (
              <button
                onClick={loadNextPage}
                className="mt-1 text-xs text-neutral-400 hover:text-white px-4 py-1.5 rounded-lg border border-neutral-800 hover:bg-neutral-800 transition-colors press-feedback-sm"
              >
                加载更多歌曲 ({playlist.tracks.items.length} / {playlist.tracks.total})
              </button>
            )}
          </div>
        </div>
      ) : null}
    </div>
  );
}
