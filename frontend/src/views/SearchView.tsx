import { useState, useEffect } from 'react';
import { Search, RefreshCw, AlertCircle } from 'lucide-react';
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

  useEffect(() => {
    if (!searchQuery.trim()) {
      setTracks([]);
      setTotal(0);
      return;
    }

    setLoading(true);
    setError(null);

    searchMusic(searchQuery.trim(), 0, 50)
      .then((page) => {
        setTracks(page.items);
        setTotal(page.total);
      })
      .catch((err) => {
        console.error('Search failed:', err);
        setError('搜索请求失败，请检查网络后重试');
      })
      .finally(() => setLoading(false));
  }, [searchQuery]);

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
        <SongTable tracks={tracks} />
      )}
    </div>
  );
}
