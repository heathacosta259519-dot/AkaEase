import { useState, useEffect } from 'react';
import { ChevronLeft, ChevronRight, Search, X } from 'lucide-react';
import { useViewStore, viewActions } from '../../stores/viewStore';
import { useSessionStore } from '../../stores/sessionStore';

export function Header() {
  const currentView = useViewStore((s) => s.currentView);
  const history = useViewStore((s) => s.history);
  const historyIndex = useViewStore((s) => s.historyIndex);
  const searchQuery = useViewStore((s) => s.searchQuery);
  const activePlaylistId = useViewStore((s) => s.activePlaylistId);
  const userPlaylists = useSessionStore((s) => s.userPlaylists);
  const likedPlaylistId =
    userPlaylists?.likedPlaylistId ??
    userPlaylists?.items.find((p) => p.isLikedPlaylist)?.id ??
    null;

  const [inputVal, setInputVal] = useState(searchQuery);

  useEffect(() => {
    setInputVal(searchQuery);
  }, [searchQuery]);

  const canBack = historyIndex > 0;
  const canForward = historyIndex < history.length - 1;

  const handleSearchSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (inputVal.trim()) {
      viewActions.setSearchQuery(inputVal.trim());
      viewActions.navigate('search');
    }
  };

  const handleClear = () => {
    setInputVal('');
    viewActions.setSearchQuery('');
  };

  return (
    <header
      data-tauri-drag-region
      className="flex h-14 items-center justify-between border-b border-neutral-800/80 bg-neutral-950 px-6 select-none z-10"
    >
      {/* Navigation history controls */}
      <div className="flex items-center gap-2" data-tauri-drag-region="false">
        <button
          onClick={() => viewActions.back()}
          disabled={!canBack}
          aria-label="后退"
          className="flex h-8 w-8 items-center justify-center rounded-lg border border-neutral-800 text-neutral-400 transition-colors enabled:hover:bg-neutral-800 enabled:hover:text-neutral-100 disabled:opacity-30 press-feedback-sm"
        >
          <ChevronLeft className="h-4.5 w-4.5" />
        </button>
        <button
          onClick={() => viewActions.forward()}
          disabled={!canForward}
          aria-label="前进"
          className="flex h-8 w-8 items-center justify-center rounded-lg border border-neutral-800 text-neutral-400 transition-colors enabled:hover:bg-neutral-800 enabled:hover:text-neutral-100 disabled:opacity-30 press-feedback-sm"
        >
          <ChevronRight className="h-4.5 w-4.5" />
        </button>
      </div>

      {/* Global Search Bar */}
      <form onSubmit={handleSearchSubmit} className="relative w-84">
        <Search className="absolute left-3.5 top-1/2 h-4 w-4 -translate-y-1/2 text-neutral-500 transition-colors" />
        <input
          type="text"
          value={inputVal}
          onChange={(e) => setInputVal(e.target.value)}
          placeholder="搜索单曲、歌手、专辑..."
          className="h-8.5 w-full rounded-full border border-neutral-800 bg-neutral-900/90 pl-10 pr-9 text-sm text-neutral-100 placeholder-neutral-500 transition-all duration-200 focus:border-rose-500/80 focus:ring-2 focus:ring-rose-500/20 focus:bg-neutral-900 focus:outline-none"
        />
        {inputVal && (
          <button
            type="button"
            onClick={handleClear}
            className="absolute right-3 top-1/2 -translate-y-1/2 p-0.5 text-neutral-500 hover:text-neutral-200 press-feedback-sm"
          >
            <X className="h-4 w-4" />
          </button>
        )}
      </form>

      {/* Top right meta information */}
      <div className="flex items-center gap-3 text-sm text-neutral-400">
        <span className="capitalize text-neutral-500">
          {currentView === 'discover' && '发现音乐'}
          {currentView === 'daily' && '每日推荐'}
          {currentView === 'liked' && '我喜欢的音乐'}
          {currentView === 'playlist' &&
            (activePlaylistId === likedPlaylistId ? '我喜欢的音乐' : '歌单详情')}
          {currentView === 'artist' && '歌手详情'}
          {currentView === 'album' && '专辑详情'}
          {currentView === 'search' && '搜索结果'}
          {currentView === 'settings' && '系统设置'}
        </span>
      </div>
    </header>
  );
}
