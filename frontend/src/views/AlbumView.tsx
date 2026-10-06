import { useState, useEffect } from 'react';
import { Play, Disc3, RefreshCw, AlertCircle, Clock, Calendar, Building2, ChevronDown, ChevronUp } from 'lucide-react';
import { useViewStore, viewActions } from '../stores/viewStore';
import { getAlbumDetail } from '../services/api';
import type { AlbumDetail } from '../types/backend';
import { SongTable } from '../components/music/SongTable';
import { playerActions } from '../stores/playerStore';

export function AlbumView() {
  const activeAlbumId = useViewStore((s) => s.activeAlbumId);
  const [album, setAlbum] = useState<AlbumDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [descExpanded, setDescExpanded] = useState(false);

  const fetchAlbum = () => {
    if (!activeAlbumId) return;
    setLoading(true);
    setError(null);

    getAlbumDetail(activeAlbumId)
      .then((data) => {
        setAlbum(data);
      })
      .catch((err) => {
        console.error('Failed to load album:', err);
        setError('无法加载专辑详情，请检查网络或重试');
      })
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    fetchAlbum();
    setDescExpanded(false);
  }, [activeAlbumId]);

  const handlePlayAll = () => {
    if (album && album.tracks.length > 0) {
      playerActions.replaceQueue(album.tracks, 0, true);
    }
  };

  if (!activeAlbumId) {
    return (
      <div className="flex h-64 items-center justify-center text-xs text-neutral-500">
        请选择一张专辑
      </div>
    );
  }

  const totalDurationMs = album?.tracks.reduce((acc, t) => acc + t.durationMs, 0) ?? 0;

  // Format publish date
  const publishDate = album?.publishTimeMs
    ? new Date(album.publishTimeMs).toLocaleDateString('zh-CN', {
        year: 'numeric',
        month: '2-digit',
        day: '2-digit',
      })
    : null;

  return (
    <div className="space-y-6 max-w-6xl mx-auto select-none">
      {/* Immersive Album Header Hero Banner */}
      {album && (
        <div className="relative overflow-hidden rounded-2xl bg-neutral-900/60 border border-neutral-800/80 p-8 shadow-xl">
          {/* Subtle Ambient Radial Gradient */}
          {album.coverUrl && (
            <div
              className="absolute -right-16 -top-16 h-96 w-96 rounded-full pointer-events-none opacity-20"
              style={{
                background: 'radial-gradient(circle, rgba(244, 63, 94, 0.4) 0%, rgba(244, 63, 94, 0) 70%)',
              }}
            />
          )}

          <div className="relative z-10 flex flex-col sm:flex-row items-center sm:items-end gap-7">
            {/* Vinyl-style Artwork */}
            <div className="relative h-44 w-44 overflow-hidden rounded-2xl bg-neutral-800 shadow-2xl ring-1 ring-white/10 shrink-0 group">
              {album.coverUrl ? (
                <img
                  src={album.coverUrl}
                  alt={album.name}
                  className="h-full w-full object-cover transition-transform duration-500 group-hover:scale-105"
                />
              ) : (
                <div className="flex h-full w-full items-center justify-center text-neutral-600">
                  <Disc3 className="h-16 w-16" />
                </div>
              )}
            </div>

            {/* Album Meta Details */}
            <div className="flex flex-col gap-2.5 flex-1 min-w-0 text-center sm:text-left">
              <div className="flex items-center justify-center sm:justify-start gap-2">
                <span className="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full bg-rose-500/15 text-[10px] font-semibold text-rose-400 border border-rose-500/20 uppercase tracking-wider">
                  <Disc3 className="h-3 w-3" />
                  专辑
                </span>
                {album.company && (
                  <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-neutral-800 text-[10px] text-neutral-400 border border-neutral-700/50 truncate max-w-xs">
                    <Building2 className="h-3 w-3 text-neutral-500" />
                    {album.company}
                  </span>
                )}
              </div>

              <h1 className="text-2xl sm:text-3xl font-extrabold text-white tracking-tight truncate">
                {album.name}
              </h1>

              {/* Artists & Publish Details */}
              <div className="flex flex-wrap items-center justify-center sm:justify-start gap-x-4 gap-y-1 text-xs text-neutral-400 font-medium pt-0.5">
                <div className="flex items-center gap-1.5">
                  <span className="text-neutral-500">艺人:</span>
                  <div className="inline-flex items-center gap-1 flex-wrap">
                    {album.artists && album.artists.length > 0 ? (
                      album.artists.map((artist, idx) => (
                        <span key={artist.id || idx} className="inline-flex items-center">
                          <button
                            onClick={() => viewActions.openArtist(artist.id)}
                            className="text-neutral-200 hover:text-rose-400 hover:underline transition-colors cursor-pointer"
                          >
                            {artist.name}
                          </button>
                          {idx < album.artists.length - 1 && <span className="text-neutral-600 ml-1">/</span>}
                        </span>
                      ))
                    ) : (
                      <span className="text-neutral-300">{album.artist?.name || '未知艺人'}</span>
                    )}
                  </div>
                </div>

                {publishDate && (
                  <span className="flex items-center gap-1 text-neutral-400">
                    <Calendar className="h-3.5 w-3.5 text-neutral-500" />
                    {publishDate}
                  </span>
                )}

                <span className="text-neutral-300">
                  共 <strong className="text-white font-semibold">{album.tracks.length}</strong> 首曲目
                </span>

                {totalDurationMs > 0 && (
                  <span className="flex items-center gap-1 text-neutral-400">
                    <Clock className="h-3.5 w-3.5 text-neutral-500" />
                    约 {Math.round(totalDurationMs / 60000)} 分钟
                  </span>
                )}
              </div>

              {/* Action Buttons */}
              <div className="pt-3 flex items-center justify-center sm:justify-start gap-3">
                <button
                  onClick={handlePlayAll}
                  disabled={album.tracks.length === 0}
                  className="inline-flex items-center gap-2 rounded-xl bg-gradient-to-r from-rose-600 to-rose-500 px-6 py-2.5 text-xs font-semibold text-white shadow-lg shadow-rose-950/60 hover:brightness-110 active:scale-95 disabled:opacity-40 transition-all cursor-pointer"
                >
                  <Play className="h-4 w-4 fill-white" />
                  播放全部
                </button>
              </div>
            </div>
          </div>

          {/* Album Description Drawer/Fold */}
          {album.description && (
            <div className="mt-6 pt-5 border-t border-neutral-800/80">
              <div
                className={`text-xs text-neutral-400 leading-relaxed whitespace-pre-line transition-all duration-300 ${
                  descExpanded ? '' : 'line-clamp-2'
                }`}
              >
                {album.description}
              </div>
              <button
                onClick={() => setDescExpanded(!descExpanded)}
                className="mt-1.5 inline-flex items-center gap-1 text-[11px] text-neutral-400 hover:text-white transition-colors cursor-pointer"
              >
                {descExpanded ? (
                  <>
                    收起介绍 <ChevronUp className="h-3 w-3" />
                  </>
                ) : (
                  <>
                    展开完整介绍 <ChevronDown className="h-3 w-3" />
                  </>
                )}
              </button>
            </div>
          )}
        </div>
      )}

      {/* Main Tracklist Content Area */}
      <div className="space-y-4">
        {loading ? (
          <div className="flex h-64 flex-col items-center justify-center gap-3 text-neutral-500">
            <RefreshCw className="h-6 w-6 animate-spin text-rose-500" />
            <p className="text-xs">加载专辑曲目中...</p>
          </div>
        ) : error ? (
          <div className="flex h-64 flex-col items-center justify-center gap-3 text-neutral-500">
            <AlertCircle className="h-8 w-8 text-rose-500/80" />
            <p className="text-xs text-rose-300">{error}</p>
            <button
              onClick={fetchAlbum}
              className="mt-2 flex items-center gap-1.5 rounded-lg border border-neutral-700 bg-neutral-800 px-3 py-1.5 text-xs text-neutral-200 hover:bg-neutral-700 transition-colors"
            >
              <RefreshCw className="h-3.5 w-3.5" />
              重新加载
            </button>
          </div>
        ) : album && album.tracks.length > 0 ? (
          <SongTable
            tracks={album.tracks}
            showAlbum={false}
            onPlayAll={handlePlayAll}
          />
        ) : (
          <div className="flex h-48 flex-col items-center justify-center gap-2 rounded-xl border border-dashed border-neutral-800/80 text-neutral-500">
            <p className="text-xs">该专辑暂无曲目</p>
          </div>
        )}
      </div>
    </div>
  );
}
