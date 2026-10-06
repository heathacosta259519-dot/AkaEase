import { useState, useEffect, useRef, useCallback } from 'react';
import {
  User,
  RefreshCw,
  AlertCircle,
  Music,
  Disc,
  Info,
  ChevronDown,
  ChevronUp,
  Play,
  Calendar,
  Sparkles,
  Loader2,
} from 'lucide-react';
import { useViewStore, viewActions } from '../stores/viewStore';
import { getArtistDetail, getArtistSongs, getArtistAlbums } from '../services/api';
import type { ArtistDetail, Track, AlbumSummary } from '../types/backend';
import { SongTable } from '../components/music/SongTable';
import { playerActions } from '../stores/playerStore';

type ArtistTab = 'songs' | 'albums';

export function ArtistView() {
  const activeArtistId = useViewStore((s) => s.activeArtistId);

  // Artist Detail State
  const [artist, setArtist] = useState<ArtistDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [descExpanded, setDescExpanded] = useState(false);
  const [activeTab, setActiveTab] = useState<ArtistTab>('songs');

  // Top Songs State
  const [songs, setSongs] = useState<Track[]>([]);
  const [songsLoading, setSongsLoading] = useState(false);
  const [songsError, setSongsError] = useState<string | null>(null);

  // Albums State
  const [albums, setAlbums] = useState<AlbumSummary[]>([]);
  const [albumsTotal, setAlbumsTotal] = useState(0);
  const [albumsHasMore, setAlbumsHasMore] = useState(false);
  const [albumsLoading, setAlbumsLoading] = useState(false);
  const [albumsLoadingMore, setAlbumsLoadingMore] = useState(false);
  const [albumsError, setAlbumsError] = useState<string | null>(null);

  const requestGenRef = useRef(0);
  const albumsLoadingMoreRef = useRef(false);
  const albumSentinelRef = useRef<HTMLDivElement>(null);

  // Fetch Artist Details & Top Songs
  const fetchArtist = () => {
    if (!activeArtistId) return;
    setLoading(true);
    setError(null);
    setSongsLoading(true);
    setSongsError(null);
    setAlbums([]);
    setAlbumsHasMore(false);
    const currentGen = ++requestGenRef.current;

    // 1. Fetch Detail
    getArtistDetail(activeArtistId)
      .then((data) => {
        if (currentGen === requestGenRef.current) {
          setArtist(data);
        }
      })
      .catch((err) => {
        if (currentGen === requestGenRef.current) {
          console.error('Failed to load artist:', err);
          setError('无法加载歌手详情，请检查网络或重试');
        }
      })
      .finally(() => {
        if (currentGen === requestGenRef.current) {
          setLoading(false);
        }
      });

    // 2. Fetch Top Songs
    getArtistSongs(activeArtistId)
      .then((data) => {
        if (currentGen === requestGenRef.current) {
          setSongs(data);
        }
      })
      .catch((err) => {
        if (currentGen === requestGenRef.current) {
          console.error('Failed to load artist songs:', err);
          setSongsError('获取热门单曲失败');
        }
      })
      .finally(() => {
        if (currentGen === requestGenRef.current) {
          setSongsLoading(false);
        }
      });
  };

  useEffect(() => {
    setActiveTab('songs');
    setDescExpanded(false);
    fetchArtist();
  }, [activeArtistId]);

  // Lazy Fetch Albums when switching to albums tab
  const fetchAlbums = useCallback(
    async (offset = 0) => {
      if (!activeArtistId) return;
      const currentGen = requestGenRef.current;

      if (offset === 0) {
        setAlbumsLoading(true);
        setAlbumsError(null);
      } else {
        albumsLoadingMoreRef.current = true;
        setAlbumsLoadingMore(true);
      }

      try {
        const page = await getArtistAlbums(activeArtistId, offset, 30);
        if (currentGen !== requestGenRef.current) return;

        setAlbums((prev) => {
          if (offset === 0) return page.items;
          const existingIds = new Set(prev.map((a) => a.id));
          const newItems = page.items.filter((a) => !existingIds.has(a.id));
          return [...prev, ...newItems];
        });
        setAlbumsTotal(page.total);
        setAlbumsHasMore(page.hasMore);
      } catch (err) {
        if (currentGen !== requestGenRef.current) return;
        console.error('Failed to load artist albums:', err);
        setAlbumsError('获取专辑列表失败，请重试');
      } finally {
        if (currentGen === requestGenRef.current) {
          setAlbumsLoading(false);
          albumsLoadingMoreRef.current = false;
          setAlbumsLoadingMore(false);
        }
      }
    },
    [activeArtistId]
  );

  useEffect(() => {
    if (activeTab === 'albums' && albums.length === 0 && !albumsLoading) {
      fetchAlbums(0);
    }
  }, [activeTab, albums.length, albumsLoading, fetchAlbums]);

  // Albums Infinite Scroll Sentinel
  useEffect(() => {
    const sentinel = albumSentinelRef.current;
    if (!sentinel || !albumsHasMore || activeTab !== 'albums') return;

    const observer = new IntersectionObserver(
      (entries) => {
        const first = entries[0];
        if (first && first.isIntersecting && !albumsLoadingMoreRef.current && !albumsLoading) {
          fetchAlbums(albums.length);
        }
      },
      { rootMargin: '300px' }
    );

    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [activeTab, albumsHasMore, albums.length, albumsLoading, fetchAlbums]);

  const handlePlayAllSongs = () => {
    if (songs.length > 0) {
      playerActions.replaceQueue(songs, 0, true);
    }
  };

  if (!activeArtistId) {
    return (
      <div className="flex h-64 items-center justify-center text-xs text-neutral-500">
        请选择一位歌手
      </div>
    );
  }

  return (
    <div className="space-y-6 max-w-6xl mx-auto select-none">
      {/* Immersive Artist Header Hero Banner */}
      {artist && (
        <div className="relative overflow-hidden rounded-2xl bg-neutral-900/60 border border-neutral-800/80 p-8 shadow-xl">
          {/* Subtle Ambient Radial Gradient */}
          {artist.coverUrl && (
            <div
              className="absolute -right-16 -top-16 h-96 w-96 rounded-full pointer-events-none opacity-20"
              style={{
                background: 'radial-gradient(circle, rgba(225, 29, 72, 0.45) 0%, rgba(225, 29, 72, 0) 70%)',
              }}
            />
          )}

          <div className="relative z-10 flex flex-col sm:flex-row items-center sm:items-end gap-7">
            {/* Artist Avatar Artwork */}
            <div className="relative h-44 w-44 overflow-hidden rounded-full bg-neutral-800 shadow-2xl ring-2 ring-white/10 shrink-0 group">
              {artist.coverUrl ? (
                <img
                  src={artist.coverUrl}
                  alt={artist.name}
                  className="h-full w-full object-cover transition-transform duration-500 group-hover:scale-105"
                />
              ) : (
                <div className="flex h-full w-full items-center justify-center text-neutral-600">
                  <User className="h-16 w-16" />
                </div>
              )}
            </div>

            {/* Artist Meta Information */}
            <div className="flex flex-col gap-2.5 flex-1 min-w-0 text-center sm:text-left">
              <div className="flex items-center justify-center sm:justify-start gap-2">
                <span className="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full bg-rose-500/15 text-[10px] font-semibold text-rose-400 border border-rose-500/20 uppercase tracking-wider">
                  <User className="h-3 w-3" />
                  音乐人
                </span>
                {artist.aliases && artist.aliases.length > 0 && (
                  <span className="inline-flex items-center gap-1 px-2.5 py-0.5 rounded-full bg-neutral-800 text-[10px] text-neutral-400 border border-neutral-700/50 truncate max-w-xs">
                    {artist.aliases.join(' / ')}
                  </span>
                )}
              </div>

              <h1 className="text-2xl sm:text-3xl font-extrabold text-white tracking-tight truncate">
                {artist.name}
              </h1>

              {/* Statistics & Counts */}
              <div className="flex flex-wrap items-center justify-center sm:justify-start gap-4 text-xs text-neutral-400 font-medium pt-1">
                <div className="flex items-center gap-1.5 text-neutral-300">
                  <Music className="h-3.5 w-3.5 text-rose-400" />
                  <span>单曲</span>
                  <strong className="text-white font-semibold">{artist.musicSize}</strong>
                </div>

                <div className="flex items-center gap-1.5 text-neutral-300">
                  <Disc className="h-3.5 w-3.5 text-rose-400" />
                  <span>专辑</span>
                  <strong className="text-white font-semibold">{artist.albumSize}</strong>
                </div>
              </div>
            </div>
          </div>

          {/* Artist Biography & Brief Description (Collapsible) */}
          {artist.briefDescription && (
            <div className="mt-6 pt-5 border-t border-neutral-800/80">
              <div className="flex items-center gap-1.5 text-xs font-semibold text-neutral-300 mb-2">
                <Info className="h-3.5 w-3.5 text-rose-400" />
                艺人介绍
              </div>
              <div
                className={`text-xs text-neutral-400 leading-relaxed whitespace-pre-line transition-all duration-300 ${
                  descExpanded ? '' : 'line-clamp-3'
                }`}
              >
                {artist.briefDescription}
              </div>
              {artist.briefDescription.length > 120 && (
                <button
                  onClick={() => setDescExpanded(!descExpanded)}
                  className="mt-2 inline-flex items-center gap-1 text-[11px] text-neutral-400 hover:text-white transition-colors cursor-pointer press-feedback-sm"
                >
                  {descExpanded ? (
                    <>
                      收起简介 <ChevronUp className="h-3 w-3" />
                    </>
                  ) : (
                    <>
                      展开全部简介 <ChevronDown className="h-3 w-3" />
                    </>
                  )}
                </button>
              )}
            </div>
          )}
        </div>
      )}

      {/* Navigation Segmented Tabs */}
      <div className="flex items-center gap-2 border-b border-neutral-800/80 pb-3">
        <button
          onClick={() => setActiveTab('songs')}
          className={`flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-semibold transition-all press-feedback-sm ${
            activeTab === 'songs'
              ? 'bg-rose-500/15 text-rose-400 ring-1 ring-rose-500/30'
              : 'text-neutral-400 hover:text-neutral-200 hover:bg-neutral-800/60'
          }`}
        >
          <Sparkles className="h-3.5 w-3.5" />
          <span>热门单曲</span>
          {songs.length > 0 && (
            <span className="text-[10px] px-1.5 py-0.2 rounded-full bg-neutral-800 text-neutral-400">
              {songs.length}
            </span>
          )}
        </button>

        <button
          onClick={() => setActiveTab('albums')}
          className={`flex items-center gap-2 px-4 py-2 rounded-xl text-xs font-semibold transition-all press-feedback-sm ${
            activeTab === 'albums'
              ? 'bg-rose-500/15 text-rose-400 ring-1 ring-rose-500/30'
              : 'text-neutral-400 hover:text-neutral-200 hover:bg-neutral-800/60'
          }`}
        >
          <Disc className="h-3.5 w-3.5" />
          <span>专辑作品</span>
          {artist && (
            <span className="text-[10px] px-1.5 py-0.2 rounded-full bg-neutral-800 text-neutral-400">
              {artist.albumSize}
            </span>
          )}
        </button>
      </div>

      {/* Tab 1: Top Songs Content */}
      {activeTab === 'songs' && (
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <button
                onClick={handlePlayAllSongs}
                disabled={songs.length === 0}
                className="inline-flex items-center gap-2 rounded-xl bg-gradient-to-r from-rose-600 to-rose-500 px-5 py-2 text-xs font-semibold text-white shadow-lg shadow-rose-950/60 hover:brightness-110 active:scale-95 disabled:opacity-40 transition-all cursor-pointer"
              >
                <Play className="h-3.5 w-3.5 fill-current ml-0.5" />
                <span>播放热门单曲</span>
              </button>
              <span className="text-xs text-neutral-500">
                按热度精选前 {songs.length} 首代表作
              </span>
            </div>
          </div>

          {songsLoading ? (
            <div className="flex h-48 flex-col items-center justify-center gap-3 text-neutral-500">
              <RefreshCw className="h-5 w-5 animate-spin text-rose-500" />
              <p className="text-xs">加载热门单曲中...</p>
            </div>
          ) : songsError ? (
            <div className="flex h-48 flex-col items-center justify-center gap-3 text-neutral-400">
              <AlertCircle className="h-7 w-7 text-rose-500/80" />
              <p className="text-xs text-rose-300">{songsError}</p>
              <button
                onClick={fetchArtist}
                className="rounded-lg bg-neutral-800 px-3 py-1.5 text-xs text-rose-400 hover:text-rose-300 hover:bg-neutral-700 transition-colors press-feedback-sm"
              >
                重试
              </button>
            </div>
          ) : (
            <SongTable tracks={songs} />
          )}
        </div>
      )}

      {/* Tab 2: Albums Grid Content */}
      {activeTab === 'albums' && (
        <div className="space-y-6">
          {albumsLoading ? (
            <div className="flex h-48 flex-col items-center justify-center gap-3 text-neutral-500">
              <RefreshCw className="h-5 w-5 animate-spin text-rose-500" />
              <p className="text-xs">加载专辑列表中...</p>
            </div>
          ) : albumsError ? (
            <div className="flex h-48 flex-col items-center justify-center gap-3 text-neutral-400">
              <AlertCircle className="h-7 w-7 text-rose-500/80" />
              <p className="text-xs text-rose-300">{albumsError}</p>
              <button
                onClick={() => fetchAlbums(0)}
                className="rounded-lg bg-neutral-800 px-3 py-1.5 text-xs text-rose-400 hover:text-rose-300 hover:bg-neutral-700 transition-colors press-feedback-sm"
              >
                重试
              </button>
            </div>
          ) : albums.length === 0 ? (
            <div className="flex h-48 flex-col items-center justify-center text-xs text-neutral-500 gap-2">
              <Disc className="h-8 w-8 opacity-40" />
              <span>暂无专辑作品</span>
            </div>
          ) : (
            <>
              {/* Responsive Albums Grid */}
              <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 gap-4">
                {albums.map((album) => {
                  const year = album.publishTimeMs
                    ? new Date(album.publishTimeMs).getFullYear()
                    : null;

                  return (
                    <div
                      key={album.id}
                      onClick={() => viewActions.openAlbum(album.id)}
                      className="group flex flex-col gap-2.5 p-3 rounded-2xl bg-neutral-900/40 border border-neutral-800/80 hover:border-neutral-700/80 hover:bg-neutral-800/50 hover:-translate-y-1 hover:shadow-xl hover:shadow-rose-950/20 active:scale-[0.985] transition-all duration-200 cursor-pointer"
                    >
                      {/* Album Cover */}
                      <div className="relative aspect-square w-full overflow-hidden rounded-xl bg-neutral-800 shadow-md">
                        {album.coverUrl ? (
                          <img
                            src={album.coverUrl}
                            alt={album.name}
                            className="h-full w-full object-cover transition-transform duration-300 group-hover:scale-105"
                          />
                        ) : (
                          <div className="flex h-full w-full items-center justify-center text-neutral-600">
                            <Disc className="h-10 w-10" />
                          </div>
                        )}

                        {/* Hover Play Button Overlay */}
                        <div className="absolute right-2.5 bottom-2.5 flex h-8 w-8 items-center justify-center rounded-full bg-rose-600 text-white shadow-lg opacity-0 translate-y-1 transition-all duration-200 group-hover:opacity-100 group-hover:translate-y-0 group-hover:scale-105">
                          <Play className="h-3.5 w-3.5 fill-current ml-0.5" />
                        </div>
                      </div>

                      {/* Album Info */}
                      <div className="flex flex-col min-w-0">
                        <span className="truncate text-xs font-semibold text-neutral-200 group-hover:text-white transition-colors">
                          {album.name}
                        </span>
                        <div className="flex items-center gap-2 mt-0.5 text-[11px] text-neutral-500">
                          {year && (
                            <span className="flex items-center gap-1">
                              <Calendar className="h-3 w-3" />
                              {year}
                            </span>
                          )}
                          <span>·</span>
                          <span>{album.trackCount} 首</span>
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>

              {/* Infinite Scroll Sentinel for Albums */}
              <div
                ref={albumSentinelRef}
                className="py-6 flex flex-col items-center justify-center text-xs text-neutral-500"
              >
                {albumsLoadingMore && (
                  <div className="flex items-center gap-2 text-rose-400 font-medium py-2">
                    <Loader2 className="h-4 w-4 animate-spin text-rose-500" />
                    <span>正在加载更多专辑 ({albums.length} / {albumsTotal})...</span>
                  </div>
                )}

                {!albumsHasMore && albums.length > 15 && (
                  <div className="text-neutral-600 text-[11px] py-2">
                    —— 已加载全部 {albums.length} 张专辑 ——
                  </div>
                )}
              </div>
            </>
          )}
        </div>
      )}

      {/* Global Loading & Error States for Artist Header */}
      {loading && (
        <div className="flex h-64 flex-col items-center justify-center gap-3 text-neutral-500">
          <RefreshCw className="h-6 w-6 animate-spin text-rose-500" />
          <p className="text-xs">加载歌手主页中...</p>
        </div>
      )}

      {error && (
        <div className="flex h-64 flex-col items-center justify-center gap-3 text-neutral-500">
          <AlertCircle className="h-8 w-8 text-rose-500/80" />
          <p className="text-xs text-rose-300">{error}</p>
          <button
            onClick={fetchArtist}
            className="mt-2 flex items-center gap-1.5 rounded-lg border border-neutral-700 bg-neutral-800 px-3 py-1.5 text-xs text-neutral-200 hover:bg-neutral-700 transition-colors press-feedback-sm"
          >
            <RefreshCw className="h-3.5 w-3.5" />
            重新加载
          </button>
        </div>
      )}
    </div>
  );
}
