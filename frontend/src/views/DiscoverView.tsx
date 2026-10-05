import { Play, Sparkles, Flame, TrendingUp, Music } from 'lucide-react';
import { viewActions } from '../stores/viewStore';
import { useSessionStore, sessionActions } from '../stores/sessionStore';

const FEATURED_PLAYLISTS = [
  {
    id: '3778678',
    title: '热歌榜',
    desc: '全网最火流行金曲排行榜',
    tag: '官方榜单',
    coverColor: 'from-amber-600/30 to-rose-900/40',
    icon: Flame,
  },
  {
    id: '19723756',
    title: '飙升榜',
    desc: '100首云音乐上升最快的单曲',
    tag: '官方榜单',
    coverColor: 'from-rose-600/30 to-purple-900/40',
    icon: TrendingUp,
  },
  {
    id: '3779629',
    title: '新歌榜',
    desc: '云音乐最新最潮流行新曲',
    tag: '新歌速递',
    coverColor: 'from-blue-600/30 to-indigo-900/40',
    icon: Sparkles,
  },
  {
    id: '2884035',
    title: '原创榜',
    desc: '独立音乐人最新力作',
    tag: '原创音乐',
    coverColor: 'from-emerald-600/30 to-teal-900/40',
    icon: Music,
  },
];

export function DiscoverView() {
  const session = useSessionStore((s) => s.session);
  const profile = session?.profile;

  return (
    <div className="space-y-8 select-none max-w-6xl mx-auto">
      {/* Hero Banner */}
      <div className="relative overflow-hidden rounded-2xl bg-gradient-to-r from-rose-950/60 via-neutral-900 to-neutral-900 p-8 border border-neutral-800">
        <div className="relative z-10 max-w-lg space-y-3">
          <div className="inline-flex items-center gap-1.5 rounded-full bg-rose-500/20 px-3 py-1 text-xs font-medium text-rose-400">
            <Sparkles className="h-3.5 w-3.5" />
            <span>AkaEase Linux 原生体验</span>
          </div>
          <h1 className="text-2xl font-bold tracking-tight text-white">
            {profile ? `欢迎回来，${profile.nickname}` : '探索属于你的音乐世界'}
          </h1>
          <p className="text-xs text-neutral-400 leading-relaxed">
            基于 GStreamer 高性能音频引擎与 Linux MPRIS 系统级媒体集成，纯粹、专注、丝滑。
          </p>
          <div className="pt-2 flex items-center gap-3">
            {profile ? (
              <button
                onClick={() => viewActions.navigate('daily')}
                className="inline-flex items-center gap-2 rounded-lg bg-rose-600 px-4 py-2 text-xs font-medium text-white shadow-md shadow-rose-900/30 hover:bg-rose-500 transition-colors"
              >
                <Play className="h-3.5 w-3.5 fill-current" />
                <span>开启今日推荐</span>
              </button>
            ) : (
              <button
                onClick={() => sessionActions.openLoginModal()}
                className="inline-flex items-center gap-2 rounded-lg bg-rose-600 px-4 py-2 text-xs font-medium text-white shadow-md shadow-rose-900/30 hover:bg-rose-500 transition-colors"
              >
                <span>扫码登录查看个人曲库</span>
              </button>
            )}
          </div>
        </div>

        <div
          className="absolute -right-8 -bottom-8 h-64 w-64 rounded-full pointer-events-none"
          style={{
            background: 'radial-gradient(circle, rgba(225, 29, 72, 0.15) 0%, rgba(225, 29, 72, 0) 70%)',
          }}
        />
      </div>

      {/* Featured Playlists Section */}
      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <h2 className="text-base font-semibold text-neutral-100">官方精选榜单</h2>
          <span className="text-xs text-neutral-500">实时更新</span>
        </div>

        <div className="grid grid-cols-2 gap-4 sm:grid-cols-4">
          {FEATURED_PLAYLISTS.map((pl) => {
            const Icon = pl.icon;
            return (
              <div
                key={pl.id}
                onClick={() => viewActions.openPlaylist(pl.id)}
                className="group relative flex flex-col justify-between overflow-hidden rounded-xl border border-neutral-800/80 bg-neutral-900/50 p-4 transition-all duration-200 hover:-translate-y-0.5 hover:border-neutral-700 hover:bg-neutral-900 cursor-pointer shadow-sm"
              >
                <div className={`absolute inset-0 bg-gradient-to-br ${pl.coverColor} opacity-20 transition-opacity group-hover:opacity-40`} />

                <div className="relative z-10 space-y-2">
                  <div className="flex items-center justify-between">
                    <span className="text-[10px] font-semibold tracking-wider text-rose-400 uppercase">
                      {pl.tag}
                    </span>
                    <Icon className="h-4 w-4 text-neutral-400 group-hover:text-rose-400 transition-colors" />
                  </div>
                  <h3 className="text-base font-semibold text-neutral-100 group-hover:text-white">
                    {pl.title}
                  </h3>
                  <p className="text-xs text-neutral-400 line-clamp-2 leading-relaxed">
                    {pl.desc}
                  </p>
                </div>

                <div className="relative z-10 pt-4 flex items-center justify-end">
                  <div className="flex h-8 w-8 items-center justify-center rounded-full bg-neutral-800 text-neutral-200 opacity-0 shadow transition-all duration-200 group-hover:opacity-100 group-hover:scale-105 group-hover:bg-rose-600 group-hover:text-white">
                    <Play className="h-3.5 w-3.5 fill-current ml-0.5" />
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
