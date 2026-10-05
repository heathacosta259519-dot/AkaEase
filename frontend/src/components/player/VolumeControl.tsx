import { useState, useRef } from 'react';
import { Volume2, Volume1, VolumeX } from 'lucide-react';
import { usePlayerStore, playerActions } from '../../stores/playerStore';

export function VolumeControl() {
  const snapshot = usePlayerStore((s) => s.snapshot);
  const volume = snapshot?.playback.volume ?? 1;
  const [prevVolume, setPrevVolume] = useState(volume > 0 ? volume : 1);
  const sliderRef = useRef<HTMLInputElement>(null);

  const toggleMute = () => {
    if (volume > 0) {
      setPrevVolume(volume);
      playerActions.setVolume(0);
    } else {
      playerActions.setVolume(prevVolume || 0.8);
    }
  };

  const handleSliderChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = parseFloat(e.target.value);
    playerActions.setVolume(val);
  };

  const Icon = volume === 0 ? VolumeX : volume < 0.5 ? Volume1 : Volume2;

  return (
    <div className="flex items-center gap-1.5 text-neutral-400 select-none">
      <button
        onClick={toggleMute}
        title={volume === 0 ? '取消静音' : '静音'}
        className="p-1 hover:text-neutral-100 rounded transition-colors"
      >
        <Icon className="h-4 w-4" />
      </button>

      <div className="relative flex items-center w-20">
        <input
          ref={sliderRef}
          type="range"
          min="0"
          max="1"
          step="0.01"
          value={volume}
          onChange={handleSliderChange}
          aria-label="音量调节"
          className="h-1 w-full cursor-pointer appearance-none rounded-full bg-neutral-800 accent-rose-500 hover:bg-neutral-700"
        />
      </div>
    </div>
  );
}
