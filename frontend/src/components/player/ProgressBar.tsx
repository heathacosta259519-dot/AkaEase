import { useRef, useCallback, useEffect } from 'react';
import { usePlayerStore, playerActions, getCurrentPositionMs } from '../../stores/playerStore';
import { formatDuration } from '../../utils/format';

interface ProgressBarProps {
  className?: string;
  isFullWidth?: boolean;
}

export function ProgressBar({ className = '', isFullWidth = false }: ProgressBarProps) {
  const positionMs = usePlayerStore((s) => s.positionMs);
  const snapshot = usePlayerStore((s) => s.snapshot);
  const isScrubbing = usePlayerStore((s) => s.isScrubbing);
  const scrubPositionMs = usePlayerStore((s) => s.scrubPositionMs);

  const durationMs = snapshot?.playback.durationMs ?? 0;
  const bufferingPercent = snapshot?.bufferingPercent ?? null;
  const canSeek = snapshot?.canSeek ?? false;

  const barRef = useRef<HTMLDivElement>(null);
  const fillRef = useRef<HTMLDivElement>(null);
  const thumbRef = useRef<HTMLDivElement>(null);
  const timeRef = useRef<HTMLSpanElement>(null);

  const currentDisplayMs = isScrubbing ? scrubPositionMs : positionMs;
  const progressRatio = durationMs > 0 ? Math.min(1, Math.max(0, currentDisplayMs / durationMs)) : 0;

  useEffect(() => {
    if (snapshot?.playback.state !== 'playing' || isScrubbing || durationMs <= 0) return;

    const updateProgress = () => {
      const position = getCurrentPositionMs();
      const percent = `${Math.min(100, Math.max(0, position / durationMs * 100))}%`;
      if (fillRef.current) fillRef.current.style.width = percent;
      if (thumbRef.current) thumbRef.current.style.left = percent;
      if (timeRef.current) timeRef.current.textContent = formatDuration(position);
    };
    updateProgress();
    const timer = window.setInterval(updateProgress, 100);
    return () => window.clearInterval(timer);
  }, [snapshot?.playback.state, durationMs, positionMs, isScrubbing]);

  const calculatePositionFromMouseEvent = useCallback(
    (e: MouseEvent): number => {
      if (!barRef.current || durationMs <= 0) return 0;
      const rect = barRef.current.getBoundingClientRect();
      const clickX = e.clientX - rect.left;
      const clampedRatio = Math.max(0, Math.min(1, clickX / rect.width));
      return clampedRatio * durationMs;
    },
    [durationMs],
  );

  const handleMouseDown = (e: React.MouseEvent) => {
    if (!canSeek || durationMs <= 0) return;
    const targetMs = calculatePositionFromMouseEvent(e.nativeEvent);
    playerActions.startScrubbing(targetMs);

    const handleMouseMove = (moveEvent: MouseEvent) => {
      const pos = calculatePositionFromMouseEvent(moveEvent);
      playerActions.updateScrubbing(pos);
    };

    const handleMouseUp = (upEvent: MouseEvent) => {
      window.removeEventListener('mousemove', handleMouseMove);
      window.removeEventListener('mouseup', handleMouseUp);
      const finalPos = calculatePositionFromMouseEvent(upEvent);
      playerActions.seek(finalPos);
    };

    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', handleMouseUp);
  };

  // Prevent drag selections during scrubbing
  useEffect(() => {
    if (isScrubbing) {
      document.body.style.userSelect = 'none';
      return () => {
        document.body.style.userSelect = '';
      };
    }
  }, [isScrubbing]);

  return (
    <div className={`flex w-full ${isFullWidth ? 'max-w-none' : 'max-w-lg'} items-center gap-3 select-none ${className}`}>
      <span ref={timeRef} className="w-11 text-right font-mono text-xs font-medium text-neutral-400 shrink-0">
        {formatDuration(currentDisplayMs)}
      </span>

      <div
        ref={barRef}
        onMouseDown={handleMouseDown}
        className={`group relative flex h-4 flex-1 items-center cursor-pointer ${
          !canSeek ? 'opacity-40 pointer-events-none' : ''
        }`}
      >
        {/* Track background */}
        <div className="relative h-1 w-full overflow-hidden rounded-full bg-neutral-800 transition-all group-hover:h-1.5">
          {/* Buffering background */}
          {bufferingPercent !== null && bufferingPercent > 0 && (
            <div
              className="absolute left-0 top-0 h-full bg-neutral-700/60"
              style={{ width: `${bufferingPercent}%` }}
            />
          )}

          {/* Active progress */}
          <div
            ref={fillRef}
            className="absolute left-0 top-0 h-full bg-rose-500 rounded-full"
            style={{ width: `${progressRatio * 100}%` }}
          />
        </div>

        {/* Thumb knob */}
        <div
          ref={thumbRef}
          className="absolute h-3 w-3 -translate-x-1/2 rounded-full bg-white shadow-sm opacity-0 transition-opacity group-hover:opacity-100"
          style={{ left: `${progressRatio * 100}%` }}
        />
      </div>

      <span className="w-11 text-left font-mono text-xs font-medium text-neutral-500 shrink-0">
        {formatDuration(durationMs)}
      </span>
    </div>
  );
}
