import { 
  Play, 
  Pause, 
  SkipBack, 
  SkipForward, 
  Shuffle, 
  Repeat, 
  Repeat1, 
  ListMusic, 
  Quote, 
  Heart,
  Loader2,
  Music2
} from 'lucide-react';
import { usePlayerStore, playerActions } from '../../stores/playerStore';
import { useViewStore, viewActions } from '../../stores/viewStore';
import { useSessionStore } from '../../stores/sessionStore';
import { ProgressBar } from './ProgressBar';
import { VolumeControl } from './VolumeControl';
import type { RepeatMode } from '../../types/backend';

export function PlayerBar() {
  const snapshot = usePlayerStore((s) => s.snapshot);
  const isLyricsOpen = useViewStore((s) => s.isLyricsOpen);
  const isQueueOpen = useViewStore((s) => s.isQueueOpen);
  const likedIds = useSessionStore((s) => s.likedIds);

  const currentTrack = snapshot?.current ?? null;
  const playbackState = snapshot?.playback.state ?? 'stopped';
  const isPlaying = playbackState === 'playing';
  const isLoading = playbackState === 'loading' || (snapshot?.resolving ?? false);
  const repeat = snapshot?.repeat ?? 'off';
  const shuffle = snapshot?.shuffle ?? false;
  const isLiked = currentTrack ? likedIds.has(currentTrack.id) : false;

  const cycleRepeat = () => {
    const sequence: RepeatMode[] = ['off', 'all', 'one'];
    const nextIdx = (sequence.indexOf(repeat) + 1) % sequence.length;
    playerActions.setRepeat(sequence[nextIdx]);
  };

  const artistsText = currentTrack?.artists.map((a) => a.name).join(', ') ?? '未知歌手';

  return (
    <footer className="relative z-20 flex h-[76px] w-full items-center justify-between border-t border-neutral-800/80 bg-neutral-950 px-4 select-none">
      {/* Left: Track Information */}
      <div className="flex w-1/4 min-w-[200px] items-center gap-3">
        {currentTrack ? (
          <>
            <div 
              onClick={() => viewActions.toggleLyrics()}
              className="group relative h-12 w-12 shrink-0 cursor-pointer overflow-hidden rounded-lg bg-neutral-900 shadow-sm"
              title="点击切换全屏歌词"
            >
              {currentTrack.album.coverUrl ? (
                <img
                  src={currentTrack.album.coverUrl}
                  alt={currentTrack.title}
                  className="h-full w-full object-cover transition-transform group-hover:scale-105"
                />
              ) : (
                <div className="flex h-full w-full items-center justify-center bg-neutral-800 text-neutral-500">
                  <Music2 className="h-6 w-6" />
                </div>
              )}
              <div className="absolute inset-0 flex items-center justify-center bg-black/40 opacity-0 backdrop-blur-[2px] transition-opacity group-hover:opacity-100">
                <Quote className="h-5 w-5 text-white" />
              </div>
            </div>

            <div className="flex min-w-0 flex-col">
              <div className="flex items-center gap-1.5">
                <span className="truncate text-xs font-medium text-neutral-100">
                  {currentTrack.title}
                </span>
                {snapshot?.isPreview && (
                  <span className="shrink-0 rounded bg-amber-500/20 px-1 py-0.5 text-[10px] font-medium text-amber-300">
                    试听
                  </span>
                )}
              </div>
              <span className="truncate text-[11px] text-neutral-400">
                {artistsText}
              </span>
            </div>

            <button
              disabled
              title={isLiked ? '已喜欢' : '喜欢'}
              className={`p-1.5 transition-colors ${
                isLiked ? 'text-rose-500' : 'text-neutral-500 hover:text-neutral-300'
              }`}
            >
              <Heart className={`h-4 w-4 ${isLiked ? 'fill-rose-500' : ''}`} />
            </button>
          </>
        ) : (
          <div className="flex items-center gap-2.5 text-neutral-500">
            <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-neutral-900 border border-neutral-800/80">
              <Music2 className="h-5 w-5 text-neutral-600" />
            </div>
            <span className="text-xs">暂无播放曲目</span>
          </div>
        )}
      </div>

      {/* Center: Playback Controls & Progress Bar */}
      <div className="flex flex-1 flex-col items-center justify-center gap-1.5 px-4">
        <div className="flex items-center gap-4">
          {/* Shuffle Button */}
          <button
            onClick={() => playerActions.setShuffle(!shuffle)}
            title={shuffle ? '关闭随机播放' : '开启随机播放'}
            className={`p-1.5 transition-colors rounded ${
              shuffle ? 'text-rose-400' : 'text-neutral-400 hover:text-neutral-200'
            }`}
          >
            <Shuffle className="h-4 w-4" />
          </button>

          {/* Previous Track */}
          <button
            onClick={() => playerActions.previous()}
            disabled={!snapshot?.canPrevious}
            title="上一曲"
            className="p-1.5 text-neutral-300 transition-colors hover:text-white disabled:opacity-30 disabled:hover:text-neutral-300"
          >
            <SkipBack className="h-4 w-4 fill-current" />
          </button>

          {/* Play/Pause Main Button */}
          <button
            onClick={() => playerActions.toggle()}
            disabled={!currentTrack && (snapshot?.queueLength ?? 0) === 0}
            title={isPlaying ? '暂停' : '播放'}
            className="flex h-9 w-9 items-center justify-center rounded-full bg-rose-600 text-white shadow-md shadow-rose-900/40 transition-transform active:scale-95 hover:bg-rose-500 disabled:opacity-40"
          >
            {isLoading ? (
              <Loader2 className="h-4 w-4 animate-spin" />
            ) : isPlaying ? (
              <Pause className="h-4 w-4 fill-current" />
            ) : (
              <Play className="h-4 w-4 fill-current ml-0.5" />
            )}
          </button>

          {/* Next Track */}
          <button
            onClick={() => playerActions.next()}
            disabled={!snapshot?.canNext}
            title="下一曲"
            className="p-1.5 text-neutral-300 transition-colors hover:text-white disabled:opacity-30 disabled:hover:text-neutral-300"
          >
            <SkipForward className="h-4 w-4 fill-current" />
          </button>

          {/* Repeat Mode */}
          <button
            onClick={cycleRepeat}
            title={repeat === 'off' ? '顺序播放' : repeat === 'all' ? '列表循环' : '单曲循环'}
            className={`p-1.5 transition-colors rounded ${
              repeat !== 'off' ? 'text-rose-400' : 'text-neutral-400 hover:text-neutral-200'
            }`}
          >
            {repeat === 'one' ? <Repeat1 className="h-4 w-4" /> : <Repeat className="h-4 w-4" />}
          </button>
        </div>

        {/* Scrubbing Progress Bar */}
        <ProgressBar />
      </div>

      {/* Right: Auxiliary Controls */}
      <div className="flex w-1/4 min-w-[200px] items-center justify-end gap-3">
        {/* Toggle Fullscreen Lyrics */}
        <button
          onClick={() => viewActions.toggleLyrics()}
          title={isLyricsOpen ? '收起歌词' : '展开歌词'}
          className={`flex items-center gap-1 px-2 py-1 rounded text-xs transition-colors ${
            isLyricsOpen
              ? 'bg-rose-500/20 text-rose-400 font-medium'
              : 'text-neutral-400 hover:text-neutral-200 hover:bg-neutral-800'
          }`}
        >
          <Quote className="h-3.5 w-3.5" />
          <span>词</span>
        </button>

        {/* Volume Slider */}
        <VolumeControl />

        {/* Current Playback Queue Drawer */}
        <button
          onClick={() => viewActions.toggleQueue()}
          title="播放队列"
          className={`relative p-1.5 rounded transition-colors ${
            isQueueOpen
              ? 'bg-rose-500/20 text-rose-400'
              : 'text-neutral-400 hover:text-neutral-200 hover:bg-neutral-800'
          }`}
        >
          <ListMusic className="h-4 w-4" />
          {(snapshot?.queueLength ?? 0) > 0 && (
            <span className="absolute -top-1 -right-1 flex h-4 min-w-[16px] items-center justify-center rounded-full bg-rose-600 px-1 text-[9px] font-bold text-white">
              {snapshot?.queueLength}
            </span>
          )}
        </button>
      </div>
    </footer>
  );
}
