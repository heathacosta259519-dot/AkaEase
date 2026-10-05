import { useState, useEffect } from 'react';
import { Heart, Play, RefreshCw, AlertCircle } from 'lucide-react';
import { useSessionStore } from '../stores/sessionStore';
import { viewActions } from '../stores/viewStore';
import { getPlaylist } from '../services/api';
import type { PlaylistPage } from '../types/backend';
import { SongTable } from '../components/music/SongTable';
import { playerActions } from '../stores/playerStore';

export function LikedView() {
  const session = useSessionStore((s) => s.session);
  const userPlaylists = useSessionStore((s) => s.userPlaylists);
  const likedPlaylistId =
    userPlaylists?.likedPlaylistId ??
    userPlaylists?.items.find((p) => p.isLikedPlaylist)?.id ??
    null;

  const [playlist, setPlaylist] = useState<PlaylistPage | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

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
    try {
      const data = await getPlaylist(likedPlaylistId, 0, 100);
      setPlaylist(data);
    } catch (err) {
      console.error('Failed to load liked playlist via music_playlist:', err);
      setError('无法获取喜欢的音乐歌单，请检查网络后重试');
    } finally {
      setLoading(false);
    }
  };

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
      playerActions.replaceQueue(playlist.tracks.items, 0, true);
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
              disabled={!playlist || playlist.tracks.items.length === 0}
              className="inline-flex items-center gap-2 rounded-lg bg-rose-600 px-4 py-1.5 text-xs font-medium text-white shadow hover:bg-rose-500 disabled:opacity-40 transition-colors"
            >
              <Play className="h-3.5 w-3.5 fill-current" />
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
        <SongTable
          tracks={playlist.tracks.items}
          unavailableIds={playlist.unavailableIds}
        />
      ) : null}
    </div>
  );
}
