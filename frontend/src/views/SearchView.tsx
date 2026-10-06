import { useState, useEffect, useRef, useCallback } from 'react';
import { Search, RefreshCw, AlertCircle, Loader2 } from 'lucide-react';
import { useViewStore } from '../stores/viewStore';
import { searchMusic } from '../services/api';
import type { Track } from '../types/backend';
import { SongTable } from '../components/music/SongTable';

export function SearchView() {
  const searchQuery = useViewStore((s) => s.searchQuery);
  const [tracks, setTracks] = useState<Track[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [total, setTotal] = useState(0);
  const [hasMore, setHasMore] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [loadMoreError, setLoadMoreError] = useState<string | null>(null);

  const requestGenRef = useRef(0);
  const loadingMoreRef = useRef(false);
  const sentinelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!searchQuery.trim()) {
      setTracks([]);
      setTotal(0);
      setHasMore(false);
      return;
    }

    setLoading(true);
    setError(null);
    setLoadingMore(false);
    setLoadMoreError(null);
    const currentGen = ++requestGenRef.current;

    searchMusic(searchQuery.trim(), 0, 50)
      .then((page) => {
        if (currentGen === requestGenRef.current) {
          setTracks(page.items);
          setTotal(page.total);
          setHasMore(page.hasMore && page.items.length < page.total);
        }
      })
      .catch((err) => {
        if (currentGen === requestGenRef.current) {
          console.error('Search failed:', err);
          setError('搜索请求失败，请检查网络后重试');
        }
      })
      .finally(() => {
        if (currentGen === requestGenRef.current) {
          setLoading(false);
        }
      });
  }, [searchQuery]);

  const loadNextPage = useCallback(async () => {
    if (!searchQuery.trim() || loading || loadingMoreRef.current || !hasMore) return;
    const offset = tracks.length;
    if (offset >= total) return;

    loadingMoreRef.current = true;
    setLoadingMore(true);
    setLoadMoreError(null);
    const currentGen = requestGenRef.current;

    try {
      const nextPage = await searchMusic(searchQuery.trim(), offset, 50);
      if (currentGen !== requestGenRef.current) return;

      setTracks((prev) => {
        const existingIds = new Set(prev.map((t) => t.id));
        const incoming = nextPage.items.filter((t) => !existingIds.has(t.id));
        const merged = [...prev, ...incoming];
        setHasMore(nextPage.hasMore && incoming.length > 0 && merged.length < nextPage.total);
        return merged;
      });
    } catch (err) {
      if (currentGen !== requestGenRef.current) return;
      console.error('Failed to load more search results:', err);
      setLoadMoreError('加载后续结果失败，请点击重试');
    } finally {
      if (currentGen === requestGenRef.current) {
        loadingMoreRef.current = false;
        setLoadingMore(false);
      }
    }
  }, [searchQuery, tracks.length, total, hasMore, loading]);

  useEffect(() => {
    const sentinel = sentinelRef.current;
    if (!sentinel || !hasMore) return;

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
  }, [hasMore, loadNextPage]);

  if (!searchQuery.trim()) {
    return (
      <div className="flex h-64 flex-col items-center justify-center text-xs text-neutral-500 gap-2">
        <Search className="h-8 w-8 text-neutral-700" />
        <span>输入歌名、歌手或专辑开始搜索</span>
      </div>
    );
  }

  return (
    <div className="space-y-6 max-w-6xl mx-auto select-none">
      {/* Header */}
      <div className="flex items-center justify-between border-b border-neutral-800/80 pb-4">
        <div>
          <h1 className="text-lg font-bold text-white">
            搜索 “<span className="text-rose-400">{searchQuery}</span>”
          </h1>
          <p className="text-xs text-neutral-500 mt-1">找到 {total} 首相关单曲</p>
        </div>
      </div>

      {/* Content */}
      {loading ? (
        <div className="flex h-64 items-center justify-center text-xs text-neutral-500 gap-2">
          <RefreshCw className="h-4 w-4 animate-spin text-rose-500" />
          <span>正在搜索全网曲库...</span>
        </div>
      ) : error ? (
        <div className="flex h-64 flex-col items-center justify-center text-xs text-neutral-400 gap-2">
          <AlertCircle className="h-6 w-6 text-amber-500" />
          <span>{error}</span>
        </div>
      ) : (
        <div className="space-y-4">
          <SongTable tracks={tracks} />

          {/* Infinite Scroll Sentinel & Loading Indicator */}
          <div
            ref={sentinelRef}
            className="py-6 flex flex-col items-center justify-center text-xs text-neutral-500"
          >
            {loadingMore && (
              <div className="flex items-center gap-2 text-rose-400 font-medium py-2">
                <Loader2 className="h-4 w-4 animate-spin text-rose-500" />
                <span>正在加载更多搜索结果 ({tracks.length} / {total})...</span>
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

            {!hasMore && tracks.length > 30 && (
              <div className="text-neutral-600 text-[11px] py-2">
                —— 已显示全部 {tracks.length} 首搜索结果 ——
              </div>
            )}

            {hasMore && !loadingMore && !loadMoreError && (
              <button
                onClick={loadNextPage}
                className="mt-1 text-xs text-neutral-400 hover:text-white px-4 py-1.5 rounded-lg border border-neutral-800 hover:bg-neutral-800 transition-colors press-feedback-sm"
              >
                加载更多搜索结果 ({tracks.length} / {total})
              </button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
