import { useState, useEffect, useRef, useCallback } from 'react';
import { Heart, Play, RefreshCw, AlertCircle, Loader2 } from 'lucide-react';
import { useSessionStore } from '../stores/sessionStore';
import { viewActions } from '../stores/viewStore';
import { getPlaylist } from '../services/api';
import type { PlaylistPage } from '../types/backend';
import { SongTable } from '../components/music/SongTable';
import { playerActions, usePlayerStore } from '../stores/playerStore';

export function LikedView() {
  const session = useSessionStore((s) => s.session);
  const userPlaylists = useSessionStore((s) => s.userPlaylists);
  const likedPlaylistId =
    userPlaylists?.likedPlaylistId ??
    userPlaylists?.items.find((p) => p.isLikedPlaylist)?.id ??
    null;
  const queueLoading = usePlayerStore((s) => s.loadingPlaylistId === likedPlaylistId && likedPlaylistId !== null);
  const queueError = usePlayerStore((s) => s.playlistError?.id === likedPlaylistId ? s.playlistError.message : null);

  const [playlist, setPlaylist] = useState<PlaylistPage | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [loadingMore, setLoadingMore] = useState(false);
  const [loadMoreError, setLoadMoreError] = useState<string | null>(null);

  const requestGenRef = useRef(0);
  const loadingMoreRef = useRef(false);
  const sentinelRef = useRef<HTMLDivElement>(null);

  // If canonical liked playlist ID is already available, redirect directly to playlist view
  useEffect(() => {
    if (likedPlaylistId) {
      viewActions.openPlaylist(likedPlaylistId);
    }
  }, [likedPlaylistId]);

  // Fallback loader if rendered before redirect or if user navigated directly
  const loadLikedPlaylist = async () => {
    if (!session?.profile) {
      setLoading(false);
      setError('请登录后查看我喜欢的音乐');
      return;
    }

    if (!likedPlaylistId) {
      setLoading(false);
      setError('未找到我喜欢的音乐歌单');
      return;
    }

    setLoading(true);
    setError(null);
    setLoadingMore(false);
    setLoadMoreError(null);
    const currentGen = ++requestGenRef.current;

    try {
      const data = await getPlaylist(likedPlaylistId, 0, 100);
      if (currentGen === requestGenRef.current) {
        setPlaylist(data);
      }
    } catch (err) {
      if (currentGen === requestGenRef.current) {
        console.error('Failed to load liked playlist via music_playlist:', err);
        setError('无法获取喜欢的音乐歌单，请检查网络后重试');
      }
    } finally {
      if (currentGen === requestGenRef.current) {
        setLoading(false);
      }
    }
  };

  const loadNextPage = useCallback(async () => {
    if (!likedPlaylistId || !playlist || loading || loadingMoreRef.current) return;
    if (!playlist.tracks.hasMore) return;

    const offset = playlist.tracks.items.length;
    if (offset >= playlist.tracks.total) return;

    loadingMoreRef.current = true;
    setLoadingMore(true);
    setLoadMoreError(null);
    const currentGen = requestGenRef.current;

    try {
      const nextPage = await getPlaylist(likedPlaylistId, offset, 100);
      if (currentGen !== requestGenRef.current) return;

      setPlaylist((prev) => {
        if (!prev || prev.id !== likedPlaylistId) return prev;
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
      console.error('Failed to load more liked tracks:', err);
      setLoadMoreError('加载后续歌曲失败，请点击重试');
    } finally {
      if (currentGen === requestGenRef.current) {
        loadingMoreRef.current = false;
        setLoadingMore(false);
      }
    }
  }, [likedPlaylistId, playlist, loading]);

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

  useEffect(() => {
    if (likedPlaylistId) {
      loadLikedPlaylist();
    } else if (session?.profile && !userPlaylists) {
      setLoading(true);
    } else if (!session?.profile) {
      setLoading(false);
      setError('请登录后查看我喜欢的音乐');
    }
  }, [likedPlaylistId, session?.profile, userPlaylists]);

  const handlePlayAll = () => {
    if (playlist && playlist.tracks.items.length > 0) {
      playerActions.replacePlaylistQueue(playlist.id, playlist.tracks.items);
    }
  };

  return (
    <div className="space-y-6 max-w-6xl mx-auto select-none">
      {/* Banner */}
      <div className="flex items-center gap-6 rounded-2xl bg-neutral-900/60 border border-neutral-800/80 p-6">
        <div className="flex h-20 w-20 items-center justify-center rounded-2xl bg-gradient-to-br from-rose-600 to-rose-900 text-white shadow-lg shadow-rose-950/50 shrink-0">
          <Heart className="h-9 w-9 fill-current" />
        </div>

        <div className="flex flex-col gap-2 flex-1 min-w-0">
          <h1 className="text-xl font-bold text-white">我喜欢的音乐</h1>
          <p className="text-xs text-neutral-400">
            {playlist ? `共收录 ${playlist.tracks.total} 首单曲` : '正在同步系统喜欢歌单...'}
          </p>
          <div className="pt-1 flex items-center gap-3">
            <button
              onClick={handlePlayAll}
              disabled={!playlist || playlist.tracks.items.length === 0 || queueLoading}
              className="inline-flex items-center gap-2 rounded-lg bg-rose-600 px-4 py-1.5 text-xs font-medium text-white shadow hover:bg-rose-500 disabled:opacity-40 transition-colors"
            >
              {queueLoading ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <Play className="h-3.5 w-3.5 fill-current" />}
              <span>播放全部</span>
            </button>
            <button
              onClick={loadLikedPlaylist}
              disabled={loading}
              className="inline-flex items-center gap-1.5 rounded-lg border border-neutral-800 bg-neutral-900 px-3 py-1.5 text-xs text-neutral-400 hover:text-white hover:bg-neutral-800 transition-colors"
            >
              <RefreshCw className={`h-3 w-3 ${loading ? 'animate-spin' : ''}`} />
              <span>刷新</span>
            </button>
          </div>
        </div>
      </div>

      {/* Content */}
      {loading ? (
        <div className="flex h-64 items-center justify-center text-xs text-neutral-500 gap-2">
          <RefreshCw className="h-4 w-4 animate-spin text-rose-500" />
          <span>正在加载我喜欢的音乐歌单...</span>
        </div>
      ) : error ? (
        <div className="flex h-64 flex-col items-center justify-center text-xs text-neutral-400 gap-2">
          <AlertCircle className="h-6 w-6 text-amber-500" />
          <span>{error}</span>
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
