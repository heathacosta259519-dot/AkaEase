import { useEffect, useRef, useState } from 'react';
import {
  ChevronDown,
  Music2,
  Play,
  Pause,
  SkipBack,
  SkipForward,
  Shuffle,
  Repeat,
  Repeat1,
  Heart,
  Loader2,
  ListMusic,
} from 'lucide-react';
import { useViewStore, viewActions } from '../../stores/viewStore';
import { usePlayerStore, playerActions, getCurrentPositionMs } from '../../stores/playerStore';
import { useSessionStore } from '../../stores/sessionStore';
import { ProgressBar } from '../player/ProgressBar';
import { VolumeControl } from '../player/VolumeControl';
import { BrandLogo } from '../common/BrandLogo';
import { getLyrics } from '../../services/api';
import type { LyricLine, RepeatMode } from '../../types/backend';

export function LyricsView() {
  const isLyricsOpen = useViewStore((s) => s.isLyricsOpen);
  const snapshot = usePlayerStore((s) => s.snapshot);
  const positionMs = usePlayerStore((s) => s.positionMs);
  const likedIds = useSessionStore((s) => s.likedIds);

  const currentTrack = snapshot?.current;
  const isPlaying = snapshot?.playback.state === 'playing';
  const isLoading = snapshot?.playback.state === 'loading' || (snapshot?.resolving ?? false);
  const shuffle = snapshot?.shuffle ?? false;
  const repeat = snapshot?.repeat ?? 'off';
  const isLiked = currentTrack ? likedIds.has(currentTrack.id) : false;

  const [lyrics, setLyrics] = useState<LyricLine[]>([]);
  const [loading, setLoading] = useState(false);
  const [activeIndex, setActiveIndex] = useState(-1);

  const scrollContainerRef = useRef<HTMLDivElement>(null);

  // Fetch lyrics when track changes
  useEffect(() => {
    if (!currentTrack || !isLyricsOpen) return;
    setLoading(true);

    getLyrics(currentTrack.id)
      .then(setLyrics)
      .catch((err) => {
        console.warn('Failed to load lyrics:', err);
        setLyrics([]);
      })
      .finally(() => setLoading(false));
  }, [currentTrack?.id, isLyricsOpen]);

  const cycleRepeat = () => {
    const modes: RepeatMode[] = ['off', 'all', 'one'];
    const currentIdx = modes.indexOf(repeat);
    const nextMode = modes[(currentIdx + 1) % modes.length];
    playerActions.setRepeat(nextMode);
  };

  // Synchronize active line position
  useEffect(() => {
    if (!isLyricsOpen) return;
    const updateActiveLine = () => {
      const position = getCurrentPositionMs();
      let nextIndex = -1;
      for (let i = 0; i < lyrics.length && position >= lyrics[i].timeMs; i++) {
        nextIndex = i;
      }
      setActiveIndex((current) => (current === nextIndex ? current : nextIndex));
    };
    updateActiveLine();
    if (snapshot?.playback.state !== 'playing') return;
    const timer = window.setInterval(updateActiveLine, 150);
    return () => window.clearInterval(timer);
  }, [isLyricsOpen, lyrics, snapshot?.playback.state, positionMs]);

  // Center active lyric line automatically when activeIndex changes or when opening view
  useEffect(() => {
    if (!isLyricsOpen || lyrics.length === 0) return;

    // Immediate or rAF alignment to ensure container is laid out
    const scrollToTarget = (smooth: boolean) => {
      const container = scrollContainerRef.current;
      if (!container || activeIndex < 0) return;
      const activeEl = container.children[activeIndex] as HTMLElement | undefined;
      if (!activeEl) return;

      // 使用容器自身的 scrollTop 计算，避免 scrollIntoView 导致整页父容器甚至根视口发生滚动颠簸
      const targetScrollTop =
        activeEl.offsetTop - container.clientHeight / 2 + activeEl.clientHeight / 2;

      container.scrollTo({
        top: Math.max(0, targetScrollTop),
        behavior: smooth ? 'smooth' : 'auto',
      });
    };

    const rafId = requestAnimationFrame(() => {
      scrollToTarget(true);
    });

    return () => cancelAnimationFrame(rafId);
  }, [activeIndex, isLyricsOpen, lyrics.length]);

  if (!isLyricsOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex h-full w-full flex-col bg-[#121214] select-none overflow-hidden overscroll-none animate-lyrics-up">
      {/* Dynamic Ambient Background with Vibrant Album Artwork */}
      {currentTrack?.album.coverUrl && (
        <div
          className="absolute inset-0 bg-cover bg-center opacity-45 scale-105 pointer-events-none transition-all duration-700"
          style={{ backgroundImage: `url(${currentTrack.album.coverUrl})` }}
        />
      )}
      {/* Deep Ambient Brand Mark Watermark (Apple Music style subtle space depth) */}
      <div className="absolute inset-0 flex items-center justify-center pointer-events-none overflow-hidden">
        <BrandLogo
          variant="mark"
          className="h-[680px] w-[680px] opacity-[0.025] blur-[1px] select-none"
        />
      </div>
      {/* High-quality cinematic radial vignette preserving contrast & rich album colors */}
      <div
        className="absolute inset-0 pointer-events-none"
        style={{
          background: 'radial-gradient(circle at center, rgba(14,14,16,0.55) 0%, rgba(14,14,16,0.85) 60%, rgba(10,10,12,0.95) 100%)',
        }}
      />

      {/* Top Bar: Navigation & Meta (Draggable) */}
      <div data-tauri-drag-region className="relative z-10 flex h-14 shrink-0 items-center justify-between px-8">
        <button
          type="button"
          data-tauri-drag-region="false"
          onClick={() => viewActions.setLyricsOpen(false)}
          className="group inline-flex items-center gap-1.5 px-3 py-1.5 text-xs text-neutral-400 hover:text-white transition-colors cursor-pointer press-feedback-sm"
        >
          <ChevronDown className="h-4 w-4 transition-transform group-hover:translate-y-0.5" />
          <span>收起歌词</span>
        </button>

        <div data-tauri-drag-region className="text-center min-w-0 max-w-md">
          <h2 className="text-sm font-semibold text-neutral-200 truncate pointer-events-none">
            {currentTrack?.title ?? '未知曲目'}
          </h2>
          <p className="text-[11px] text-neutral-500 truncate pointer-events-none">
            {currentTrack?.artists.map((a) => a.name).join(' / ')}
          </p>
        </div>

        <div className="flex items-center gap-2" data-tauri-drag-region="false">
          <button
            type="button"
            onClick={() => viewActions.toggleQueue()}
            title="播放队列"
            className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs text-neutral-400 hover:text-white transition-colors cursor-pointer press-feedback-sm"
          >
            <ListMusic className="h-4 w-4" />
            <span>队列 ({snapshot?.queueLength ?? 0})</span>
          </button>
        </div>
      </div>

      {/* Center Main Stage (Split View: Disc on Left, Lyrics on Right) */}
      <div className="relative z-10 flex flex-1 overflow-hidden px-10 py-2 gap-8">
        {/* Left Side: Vinyl Disc Pure Center Stage */}
        <div className="flex w-1/2 flex-col items-center justify-center py-4">
          <div className="relative flex items-center justify-center my-auto">
            {/* Smooth Subtle Glow under the Vinyl (No dark clipping box) */}
            <div
              className="absolute h-80 w-80 rounded-full pointer-events-none"
              style={{
                background: 'radial-gradient(circle, rgba(225, 29, 72, 0.12) 0%, rgba(225, 29, 72, 0) 70%)',
              }}
            />

            {/* Vinyl Body */}
            <div
              className={`vinyl-disc-spin relative flex h-72 w-72 sm:h-84 sm:w-84 items-center justify-center rounded-full bg-neutral-950 p-2 border border-white/10 ${
                !isPlaying ? 'is-paused' : ''
              }`}
            >
              {/* Concentric Vinyl Texture */}
              <div className="absolute inset-3 rounded-full border border-neutral-800/60 pointer-events-none" />
              <div className="absolute inset-6 rounded-full border border-neutral-800/40 pointer-events-none" />
              <div className="absolute inset-10 rounded-full border border-neutral-800/30 pointer-events-none" />

              {/* Album Cover Center */}
              <div className="relative h-44 w-44 sm:h-52 sm:w-52 overflow-hidden rounded-full border-4 border-neutral-900">
                {currentTrack?.album.coverUrl ? (
                  <img
                    src={currentTrack.album.coverUrl}
                    alt={currentTrack.title}
                    className="h-full w-full object-cover"
                  />
                ) : (
                  <div className="flex h-full w-full flex-col items-center justify-center bg-gradient-to-b from-neutral-900 to-neutral-950 p-6">
                    <BrandLogo
                      variant="mark"
                      className="h-20 w-20 drop-shadow-[0_4px_16px_rgba(249,38,54,0.35)]"
                    />
                    <span className="mt-2 text-[10px] tracking-widest uppercase font-semibold text-neutral-500 font-mono">
                      AkaEase Vinyl
                    </span>
                  </div>
                )}
                {/* Spindle hole */}
                <div className="absolute inset-0 m-auto h-6 w-6 rounded-full bg-neutral-950 border-2 border-neutral-800 shadow" />
              </div>
            </div>
          </div>
        </div>

        {/* Right Side: Dual-line Synchronized High-Resolution Lyrics (Apple Music Style Smooth Typography) */}
        <div className="flex w-1/2 flex-col items-center justify-center px-4 overflow-hidden">
          <div
            ref={scrollContainerRef}
            className="h-full w-full max-w-xl overflow-y-auto space-y-8 py-48 text-center scroll-smooth scrollbar-none"
          >
            {loading ? (
              <div className="flex h-64 flex-col items-center justify-center text-xs text-neutral-500 gap-2">
                <Loader2 className="h-5 w-5 animate-spin text-rose-500" />
                <span>歌词解析中...</span>
              </div>
            ) : lyrics.length === 0 ? (
              <div className="flex h-64 flex-col items-center justify-center text-neutral-500 gap-3">
                <Music2 className="h-8 w-8 opacity-40" />
                <span className="text-sm">纯音乐，请享受旋律</span>
              </div>
            ) : (
              lyrics.map((line, idx) => {
                const isActive = idx === activeIndex;

                return (
                  <div
                    key={`${line.timeMs}-${idx}`}
                    onClick={() => playerActions.seek(line.timeMs)}
                    className={`group cursor-pointer transition-all duration-300 py-1.5 px-6 rounded-2xl ${
                      isActive
                        ? 'text-white font-semibold scale-[1.03] drop-shadow-[0_2px_12px_rgba(255,255,255,0.22)]'
                        : 'text-white/30 hover:text-white/70 hover:scale-[1.01] font-normal'
                    }`}
                  >
                    <p className="text-lg leading-relaxed tracking-wide select-text">
                      {line.text}
                    </p>
                    {line.translation && (
                      <p
                        className={`mt-2 text-lg leading-relaxed tracking-wide transition-colors select-text ${
                          isActive
                            ? 'text-rose-200/90 font-medium'
                            : 'text-white/25 group-hover:text-white/60 font-normal'
                        }`}
                      >
                        {line.translation}
                      </p>
                    )}
                  </div>
                );
              })
            )}
          </div>
        </div>
      </div>

      {/* Bottom Full-Width Playback Deck (纯粹贯穿式控制) */}
      <div className="relative z-20 flex flex-col px-8 pb-5 pt-2">
        {/* Full-Width Progress Bar */}
        <div className="w-full pb-3">
          <ProgressBar isFullWidth />
        </div>

        {/* Global Controls Row */}
        <div className="flex items-center justify-between w-full">
          {/* Left placeholder for balanced symmetry */}
          <div className="w-1/4 min-w-[140px]" />

          {/* Center: Main Playback Controls + Expanded Like Button Beside Repeat */}
          <div className="flex items-center gap-6 justify-center">
            {/* Shuffle */}
            <button
              onClick={() => playerActions.setShuffle(!shuffle)}
              title={shuffle ? '关闭随机播放' : '开启随机播放'}
              className={`p-2 transition-colors rounded-lg hover:bg-white/5 cursor-pointer ${
                shuffle ? 'text-rose-400' : 'text-neutral-400 hover:text-neutral-200'
              }`}
            >
              <Shuffle className="h-4 w-4" />
            </button>

            {/* Prev */}
            <button
              onClick={() => playerActions.previous()}
              disabled={!snapshot?.canPrevious}
              title="上一曲"
              className="p-2 text-neutral-300 transition-all hover:text-white hover:bg-white/5 rounded-lg disabled:opacity-30 active:scale-95 cursor-pointer"
            >
              <SkipBack className="h-5 w-5 fill-current" />
            </button>

            {/* Play / Pause Main Trigger */}
            <button
              onClick={() => playerActions.toggle()}
              disabled={!currentTrack && (snapshot?.queueLength ?? 0) === 0}
              title={isPlaying ? '暂停' : '播放'}
              className="flex h-11 w-11 items-center justify-center rounded-full bg-gradient-to-tr from-rose-600 to-rose-500 text-white shadow-lg shadow-rose-950 hover:brightness-110 transition-all active:scale-95 disabled:opacity-40 cursor-pointer"
            >
              {isLoading ? (
                <Loader2 className="h-5 w-5 animate-spin" />
              ) : isPlaying ? (
                <Pause className="h-5 w-5 fill-current" />
              ) : (
                <Play className="h-5 w-5 fill-current ml-0.5" />
              )}
            </button>

            {/* Next */}
            <button
              onClick={() => playerActions.next()}
              disabled={!snapshot?.canNext}
              title="下一曲"
              className="p-2 text-neutral-300 transition-all hover:text-white hover:bg-white/5 rounded-lg disabled:opacity-30 active:scale-95 cursor-pointer"
            >
              <SkipForward className="h-5 w-5 fill-current" />
            </button>

            {/* Repeat Mode */}
            <button
              onClick={cycleRepeat}
              title={repeat === 'off' ? '顺序播放' : repeat === 'all' ? '列表循环' : '单曲循环'}
              className={`p-2 transition-colors rounded-lg hover:bg-white/5 cursor-pointer ${
                repeat !== 'off' ? 'text-rose-400' : 'text-neutral-400 hover:text-neutral-200'
              }`}
            >
              {repeat === 'one' ? <Repeat1 className="h-4 w-4" /> : <Repeat className="h-4 w-4" />}
            </button>

            {/* Expanded Like Button (放到播放模式旁边，整体大一圈) */}
            <button
              disabled
              title={isLiked ? '已在红心歌单中' : '喜欢这首歌'}
              className="flex h-9 w-9 items-center justify-center rounded-full hover:bg-white/5 transition-all text-neutral-400 cursor-pointer active:scale-90"
            >
              <Heart
                className={`h-5 w-5 transition-transform ${
                  isLiked ? 'fill-rose-500 text-rose-500 scale-110' : 'hover:text-neutral-200'
                }`}
              />
            </button>
          </div>

          {/* Right: Volume */}
          <div className="flex items-center justify-end w-1/4 min-w-[140px]">
            <VolumeControl />
          </div>
        </div>
      </div>
    </div>
  );
}
