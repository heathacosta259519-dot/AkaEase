import { 
  Compass, 
  Calendar, 
  Heart, 
  Settings, 
  LogIn, 
  LogOut, 
  Music2, 
  FolderLock, 
  ListMusic,
  Radio
} from 'lucide-react';
import { useSessionStore, sessionActions } from '../../stores/sessionStore';
import { useViewStore, viewActions } from '../../stores/viewStore';

export function Sidebar() {
  const session = useSessionStore((s) => s.session);
  const userPlaylists = useSessionStore((s) => s.userPlaylists);
  const currentView = useViewStore((s) => s.currentView);
  const activePlaylistId = useViewStore((s) => s.activePlaylistId);
  const backendStatus = useViewStore((s) => s.backendStatus);

  const profile = session?.profile;
  const isSecure = session?.persistence === 'secure';

  const likedPlaylistId =
    userPlaylists?.likedPlaylistId ??
    userPlaylists?.items.find((p) => p.isLikedPlaylist)?.id ??
    null;

  const isLikedActive =
    (currentView === 'playlist' && activePlaylistId === likedPlaylistId) ||
    currentView === 'liked';

  const createdPlaylists =
    userPlaylists?.items.filter((p) => p.isCreator && !p.isLikedPlaylist) ?? [];
  const subscribedPlaylists =
    userPlaylists?.items.filter((p) => !p.isCreator && !p.isLikedPlaylist) ?? [];

  return (
    <aside className="flex h-full w-56 flex-col border-r border-neutral-800 bg-neutral-950/90 select-none text-neutral-300">
      {/* Brand Header (Draggable) */}
      <div data-tauri-drag-region className="flex h-14 items-center gap-2.5 px-4 cursor-default">
        <div className="flex h-8 w-8 items-center justify-center rounded-lg bg-rose-600 text-white shadow-md shadow-rose-900/30 pointer-events-none">
          <Music2 className="h-4 w-4" />
        </div>
        <div className="flex flex-col pointer-events-none">
          <span className="font-semibold text-sm tracking-wide text-neutral-100">AkaEase</span>
          <span className="text-[10px] text-neutral-500">Linux Native</span>
        </div>
      </div>

      {/* Main Navigation */}
      <div className="flex-1 overflow-y-auto px-2 py-3 space-y-4">
        {/* Core Nav */}
        <div className="space-y-0.5">
          <button
            onClick={() => viewActions.navigate('discover')}
            className={`flex w-full items-center gap-3 rounded-lg px-3 py-2 text-xs font-medium transition-colors ${
              currentView === 'discover'
                ? 'bg-rose-500/15 text-rose-400 font-semibold'
                : 'text-neutral-400 hover:bg-neutral-900 hover:text-neutral-200'
            }`}
          >
            <Compass className="h-4 w-4 shrink-0" />
            <span>发现音乐</span>
          </button>

          <button
            onClick={() => {
              if (!profile) {
                sessionActions.openLoginModal();
              } else {
                viewActions.navigate('daily');
              }
            }}
            className={`flex w-full items-center gap-3 rounded-lg px-3 py-2 text-xs font-medium transition-colors ${
              currentView === 'daily'
                ? 'bg-rose-500/15 text-rose-400 font-semibold'
                : 'text-neutral-400 hover:bg-neutral-900 hover:text-neutral-200'
            }`}
          >
            <Calendar className="h-4 w-4 shrink-0" />
            <span>每日推荐</span>
            {!profile && <span className="ml-auto text-[10px] text-neutral-600">需登录</span>}
          </button>

          <button
            onClick={() => {
              if (!profile) {
                sessionActions.openLoginModal();
              } else if (likedPlaylistId) {
                viewActions.openPlaylist(likedPlaylistId);
              } else {
                viewActions.navigate('liked');
              }
            }}
            className={`flex w-full items-center gap-3 rounded-lg px-3 py-2 text-xs font-medium transition-colors ${
              isLikedActive
                ? 'bg-rose-500/15 text-rose-400 font-semibold'
                : 'text-neutral-400 hover:bg-neutral-900 hover:text-neutral-200'
            }`}
          >
            <Heart className="h-4 w-4 shrink-0" />
            <span>我喜欢的音乐</span>
          </button>
        </div>

        {/* Playlists */}
        {profile && (
          <>
            {/* Created Playlists */}
            <div className="space-y-1">
              <div className="px-3 text-[11px] font-semibold tracking-wider text-neutral-500 uppercase">
                创建的歌单 ({createdPlaylists.length})
              </div>
              <div className="space-y-0.5">
                {createdPlaylists.map((pl) => {
                  const isActive = currentView === 'playlist' && activePlaylistId === pl.id;
                  return (
                    <button
                      key={pl.id}
                      onClick={() => viewActions.openPlaylist(pl.id)}
                      className={`flex w-full items-center gap-2.5 rounded-lg px-3 py-1.5 text-xs text-left transition-colors truncate ${
                        isActive
                          ? 'bg-neutral-800 text-rose-400 font-medium'
                          : 'text-neutral-400 hover:bg-neutral-900 hover:text-neutral-200'
                      }`}
                    >
                      <ListMusic className="h-3.5 w-3.5 shrink-0 text-neutral-500" />
                      <span className="truncate">{pl.title}</span>
                    </button>
                  );
                })}
              </div>
            </div>

            {/* Subscribed Playlists */}
            {subscribedPlaylists.length > 0 && (
              <div className="space-y-1">
                <div className="px-3 text-[11px] font-semibold tracking-wider text-neutral-500 uppercase">
                  收藏的歌单 ({subscribedPlaylists.length})
                </div>
                <div className="space-y-0.5">
                  {subscribedPlaylists.map((pl) => {
                    const isActive = currentView === 'playlist' && activePlaylistId === pl.id;
                    return (
                      <button
                        key={pl.id}
                        onClick={() => viewActions.openPlaylist(pl.id)}
                        className={`flex w-full items-center gap-2.5 rounded-lg px-3 py-1.5 text-xs text-left transition-colors truncate ${
                          isActive
                            ? 'bg-neutral-800 text-rose-400 font-medium'
                            : 'text-neutral-400 hover:bg-neutral-900 hover:text-neutral-200'
                        }`}
                      >
                        <ListMusic className="h-3.5 w-3.5 shrink-0 text-neutral-500" />
                        <span className="truncate">{pl.title}</span>
                      </button>
                    );
                  })}
                </div>
              </div>
            )}
          </>
        )}
      </div>

      {/* User & Settings Footer */}
      <div className="border-t border-neutral-800/80 p-2 space-y-1 bg-neutral-950/60">
        {profile ? (
          <div className="flex items-center justify-between rounded-lg px-2.5 py-1.5 bg-neutral-900/60">
            <div className="flex items-center gap-2 min-w-0">
              {profile.avatarUrl ? (
                <img
                  src={profile.avatarUrl}
                  alt={profile.nickname}
                  className="h-7 w-7 rounded-full object-cover shrink-0"
                />
              ) : (
                <div className="flex h-7 w-7 items-center justify-center rounded-full bg-neutral-800 text-neutral-300 font-medium text-xs">
                  {profile.nickname.slice(0, 1)}
                </div>
              )}
              <div className="flex flex-col min-w-0">
                <span className="text-xs font-medium text-neutral-200 truncate">
                  {profile.nickname}
                </span>
                <span className="flex items-center gap-1 text-[10px] text-neutral-500">
                  <FolderLock className={`h-2.5 w-2.5 ${isSecure ? 'text-emerald-400' : 'text-amber-400'}`} />
                  {isSecure ? '系统密钥链' : '内存会话'}
                </span>
              </div>
            </div>
            <button
              onClick={() => sessionActions.logout()}
              title="退出登录"
              className="p-1 rounded-md text-neutral-400 hover:text-rose-400 hover:bg-neutral-800 transition-colors"
            >
              <LogOut className="h-3.5 w-3.5" />
            </button>
          </div>
        ) : (
          <button
            onClick={() => sessionActions.openLoginModal()}
            className="flex w-full items-center justify-center gap-2 rounded-lg bg-rose-600/90 hover:bg-rose-600 px-3 py-1.5 text-xs font-medium text-white transition-colors shadow-sm"
          >
            <LogIn className="h-3.5 w-3.5" />
            <span>扫码登录账号</span>
          </button>
        )}

        <div className="flex items-center justify-between px-2 pt-1 text-[10px] text-neutral-500">
          <div className="flex items-center gap-1.5" title={`MPRIS: ${backendStatus?.mpris ?? 'checking'}`}>
            <Radio className={`h-3 w-3 ${backendStatus?.mpris === 'ready' ? 'text-emerald-400' : 'text-amber-500'}`} />
            <span>MPRIS</span>
          </div>

          <button
            onClick={() => viewActions.navigate('settings')}
            className={`flex items-center gap-1 px-2 py-1 rounded transition-colors ${
              currentView === 'settings'
                ? 'text-rose-400 font-medium'
                : 'text-neutral-400 hover:text-neutral-200 hover:bg-neutral-800'
            }`}
          >
            <Settings className="h-3.5 w-3.5" />
            <span>设置</span>
          </button>
        </div>
      </div>
    </aside>
  );
}
