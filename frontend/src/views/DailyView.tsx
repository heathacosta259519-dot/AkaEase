import { useState, useEffect } from 'react';
import { Calendar, Play, RefreshCw, AlertCircle } from 'lucide-react';
import { getDailyTracks } from '../services/api';
import type { Track } from '../types/backend';
import { SongTable } from '../components/music/SongTable';
import { playerActions } from '../stores/playerStore';

export function DailyView() {
  const [tracks, setTracks] = useState<Track[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadDaily = async () => {
    setLoading(true);
    setError(null);
    try {
      const data = await getDailyTracks();
      setTracks(data);
    } catch (err: unknown) {
      console.error('Failed to load daily tracks:', err);
      setError('无法获取每日推荐，请确认已登录账号');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadDaily();
  }, []);

  const today = new Date();
  const day = today.getDate();
  const month = today.getMonth() + 1;

  const handlePlayAll = () => {
    if (tracks.length > 0) {
      playerActions.replaceQueue(tracks, 0, true);
    }
  };

  return (
    <div className="space-y-6 max-w-6xl mx-auto select-none">
      {/* Banner */}
      <div className="flex items-center gap-6 rounded-2xl bg-neutral-900/60 border border-neutral-800/80 p-6">
        <div className="flex h-20 w-20 flex-col items-center justify-center rounded-2xl bg-gradient-to-br from-rose-600 to-rose-800 text-white shadow-lg shadow-rose-950/50 shrink-0">
          <Calendar className="h-4 w-4 opacity-75 mb-0.5" />
          <span className="text-xl font-bold tracking-tight">{day}</span>
          <span className="text-[10px] opacity-75">{month} 月</span>
        </div>

        <div className="flex flex-col gap-2 flex-1 min-w-0">
          <h1 className="text-xl font-bold text-white">每日歌曲推荐</h1>
          <p className="text-xs text-neutral-400">
            根据你的音乐口味生成，每天早晨 6:00 更新
          </p>
          <div className="pt-1 flex items-center gap-3">
            <button
              onClick={handlePlayAll}
              disabled={tracks.length === 0}
              className="inline-flex items-center gap-2 rounded-lg bg-rose-600 px-4 py-1.5 text-xs font-medium text-white shadow hover:bg-rose-500 disabled:opacity-40 transition-colors"
            >
              <Play className="h-3.5 w-3.5 fill-current" />
              <span>播放全部</span>
            </button>
            <button
              onClick={loadDaily}
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
          <span>正在生成专属推荐...</span>
        </div>
      ) : error ? (
        <div className="flex h-64 flex-col items-center justify-center text-xs text-neutral-400 gap-2">
          <AlertCircle className="h-6 w-6 text-amber-500" />
          <span>{error}</span>
          <button
            onClick={loadDaily}
            className="mt-2 text-rose-400 hover:underline"
          >
            重试
          </button>
        </div>
      ) : (
        <SongTable tracks={tracks} />
      )}
    </div>
  );
}
