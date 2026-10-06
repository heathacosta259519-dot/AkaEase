import { 
  Compass, 
  Calendar, 
  Heart, 
  Settings, 
  LogIn, 
  LogOut, 
  FolderLock, 
  ListMusic,
  Radio
} from 'lucide-react';
import { useSessionStore, sessionActions } from '../../stores/sessionStore';
import { useViewStore, viewActions } from '../../stores/viewStore';
import { BrandLogo } from '../common/BrandLogo';

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
    <aside className="flex h-full w-64 flex-col border-r border-neutral-800/90 bg-neutral-950/95 select-none text-neutral-300">
      {/* Brand Header (Draggable, clickable to Discover) */}
      <div
        data-tauri-drag-region
        onClick={() => viewActions.navigate('discover')}
        className="group flex h-14 items-center gap-3 px-4.5 cursor-pointer select-none border-b border-neutral-900/60"
        title="点击返回发现音乐"
      >
        {/* Brand Island Container */}
        <div className="relative flex h-8.5 w-8.5 items-center justify-center rounded-xl bg-neutral-900/90 border border-neutral-800 shadow-inner group-hover:border-rose-500/40 group-hover:shadow-[0_0_14px_rgba(249,38,54,0.3)] transition-all duration-300 press-feedback-sm">
          <BrandLogo
            variant="mark"
            className="h-5.5 w-5.5 drop-shadow-[0_1px_3px_rgba(0,0,0,0.5)] group-hover:scale-105 transition-transform duration-300"
          />
        </div>
        <div className="flex flex-col min-w-0">
          <span className="font-bold text-sm tracking-wide text-neutral-100 group-hover:text-white transition-colors">
            AkaEase
          </span>
          <span className="text-[11px] text-neutral-500 font-mono tracking-tight group-hover:text-rose-400/80 transition-colors">
            Linux Native
          </span>
        </div>
      </div>

      {/* Main Navigation */}
      <div className="flex-1 overflow-y-auto px-2.5 py-3.5 space-y-5 gpu-scroll-container">
        {/* Core Nav */}
        <div className="space-y-1">
          <button
            onClick={() => viewActions.navigate('discover')}
            className={`flex w-full items-center gap-3 rounded-xl px-3.5 py-2.5 text-sm font-medium transition-colors press-feedback-sm ${
              currentView === 'discover'
                ? 'bg-rose-500/15 text-rose-400 font-semibold'
                : 'text-neutral-400 hover:bg-neutral-900 hover:text-neutral-100'
            }`}
          >
            <Compass className="h-4.5 w-4.5 shrink-0" />
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
            className={`flex w-full items-center gap-3 rounded-xl px-3.5 py-2.5 text-sm font-medium transition-colors press-feedback-sm ${
              currentView === 'daily'
                ? 'bg-rose-500/15 text-rose-400 font-semibold'
                : 'text-neutral-400 hover:bg-neutral-900 hover:text-neutral-100'
            }`}
          >
            <Calendar className="h-4.5 w-4.5 shrink-0" />
            <span>每日推荐</span>
            {!profile && <span className="ml-auto text-xs text-neutral-500">需登录</span>}
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
            className={`flex w-full items-center gap-3 rounded-xl px-3.5 py-2.5 text-sm font-medium transition-colors press-feedback-sm ${
              isLikedActive
                ? 'bg-rose-500/15 text-rose-400 font-semibold'
                : 'text-neutral-400 hover:bg-neutral-900 hover:text-neutral-100'
            }`}
          >
            <Heart className="h-4.5 w-4.5 shrink-0" />
            <span>我喜欢的音乐</span>
          </button>
        </div>

        {/* Playlists */}
        {profile && (
          <>
            {/* Created Playlists */}
            <div className="space-y-1.5">
              <div className="px-3.5 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
                创建的歌单 ({createdPlaylists.length})
              </div>
              <div className="space-y-0.5">
                {createdPlaylists.map((pl) => {
                  const isActive = currentView === 'playlist' && activePlaylistId === pl.id;
                  return (
                    <button
                      key={pl.id}
                      onClick={() => viewActions.openPlaylist(pl.id)}
                      className={`flex w-full items-center gap-2.5 rounded-lg px-3.5 py-2 text-[13px] text-left transition-colors truncate press-feedback-sm ${
                        isActive
                          ? 'bg-neutral-800 text-rose-400 font-semibold'
                          : 'text-neutral-300 hover:bg-neutral-900 hover:text-white'
                      }`}
                    >
                      <ListMusic className="h-4 w-4 shrink-0 text-neutral-500" />
                      <span className="truncate">{pl.title}</span>
                    </button>
                  );
                })}
              </div>
            </div>

            {/* Subscribed Playlists */}
            {subscribedPlaylists.length > 0 && (
              <div className="space-y-1.5">
                <div className="px-3.5 text-xs font-semibold tracking-wider text-neutral-400 uppercase">
                  收藏的歌单 ({subscribedPlaylists.length})
                </div>
                <div className="space-y-0.5">
                  {subscribedPlaylists.map((pl) => {
                    const isActive = currentView === 'playlist' && activePlaylistId === pl.id;
                    return (
                      <button
                        key={pl.id}
                        onClick={() => viewActions.openPlaylist(pl.id)}
                        className={`flex w-full items-center gap-2.5 rounded-lg px-3.5 py-2 text-[13px] text-left transition-colors truncate press-feedback-sm ${
                          isActive
                            ? 'bg-neutral-800 text-rose-400 font-semibold'
                            : 'text-neutral-300 hover:bg-neutral-900 hover:text-white'
                        }`}
                      >
                        <ListMusic className="h-4 w-4 shrink-0 text-neutral-500" />
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
      <div className="border-t border-neutral-800/80 p-2.5 space-y-1.5 bg-neutral-950/60">
        {profile ? (
          <div className="flex items-center justify-between rounded-xl px-3 py-2 bg-neutral-900/60">
            <div className="flex items-center gap-2.5 min-w-0">
              {profile.avatarUrl ? (
                <img
                  src={profile.avatarUrl}
                  alt={profile.nickname}
                  className="h-8 w-8 rounded-full object-cover shrink-0"
                />
              ) : (
                <div className="flex h-8 w-8 items-center justify-center rounded-full bg-neutral-800 text-neutral-300 font-medium text-xs">
                  {profile.nickname.slice(0, 1)}
                </div>
              )}
              <div className="flex flex-col min-w-0">
                <span className="text-sm font-semibold text-neutral-200 truncate">
                  {profile.nickname}
                </span>
                <span className="flex items-center gap-1 text-[11px] text-neutral-500">
                  <FolderLock className={`h-3 w-3 ${isSecure ? 'text-emerald-400' : 'text-amber-400'}`} />
                  {isSecure ? '系统密钥链' : '内存会话'}
                </span>
              </div>
            </div>
            <button
              onClick={() => sessionActions.logout()}
              title="退出登录"
              className="p-1.5 rounded-lg text-neutral-400 hover:text-rose-400 hover:bg-neutral-800 transition-colors press-feedback-sm"
            >
              <LogOut className="h-4 w-4" />
            </button>
          </div>
        ) : (
          <button
            onClick={() => sessionActions.openLoginModal()}
            className="flex w-full items-center justify-center gap-2 rounded-xl bg-rose-600/90 hover:bg-rose-600 px-3.5 py-2 text-xs font-semibold text-white transition-colors shadow-sm press-feedback-sm"
          >
            <LogIn className="h-4 w-4" />
            <span>扫码登录账号</span>
          </button>
        )}

        <div className="flex items-center justify-between px-2 pt-1 text-xs text-neutral-500">
          <div className="flex items-center gap-1.5" title={`MPRIS: ${backendStatus?.mpris ?? 'checking'}`}>
            <Radio className={`h-3.5 w-3.5 ${backendStatus?.mpris === 'ready' ? 'text-emerald-400' : 'text-amber-500'}`} />
            <span>MPRIS</span>
          </div>

          <button
            onClick={() => viewActions.navigate('settings')}
            className={`flex items-center gap-1.5 px-2.5 py-1 rounded-lg transition-colors press-feedback-sm ${
              currentView === 'settings'
                ? 'text-rose-400 font-semibold'
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
