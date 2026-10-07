import { useState, useEffect } from 'react';
import { 
  Database, 
  Trash2, 
  Radio, 
  Globe, 
  Check, 
  RefreshCw, 
  Info,
  Sliders
} from 'lucide-react';
import { 
  getBackendStatus, 
  getConfig, 
  setConfig, 
  getCacheStats, 
  clearCache 
} from '../services/api';
import { playerActions } from '../stores/playerStore';
import type { BackendStatus, AppConfig, CacheStats, SoundQuality } from '../types/backend';
import { formatBytes } from '../utils/format';
import { BrandLogo } from '../components/common/BrandLogo';

export function SettingsView() {
  const [status, setStatus] = useState<BackendStatus | null>(null);
  const [config, setAppConfig] = useState<AppConfig | null>(null);
  const [cacheStats, setCache] = useState<CacheStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [savedNote, setSavedNote] = useState(false);
  const [proxyUrl, setProxyUrl] = useState('');

  const loadAll = async () => {
    setLoading(true);
    try {
      const [s, c, cs] = await Promise.all([
        getBackendStatus(),
        getConfig(),
        getCacheStats(),
      ]);
      setStatus(s);
      setAppConfig(c);
      setCache(cs);
      if (c.proxy.mode === 'http') {
        setProxyUrl(c.proxy.url);
      }
    } catch (err) {
      console.warn('Failed to load settings data:', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadAll();
  }, []);

  const handleClearCache = async () => {
    try {
      await clearCache();
      const updated = await getCacheStats();
      setCache(updated);
    } catch (err) {
      console.error('Clear cache failed:', err);
    }
  };

  const handleSaveConfig = async (newConfig: AppConfig) => {
    try {
      await setConfig(newConfig);
      setAppConfig(newConfig);
      setSavedNote(true);
      setTimeout(() => setSavedNote(false), 3000);
    } catch (err) {
      console.error('Save config failed:', err);
    }
  };

  if (loading || !config) {
    return (
      <div className="flex h-64 items-center justify-center text-xs text-neutral-500 gap-2">
        <RefreshCw className="h-4 w-4 animate-spin text-rose-500" />
        <span>正在读取系统配置...</span>
      </div>
    );
  }

  return (
    <div className="space-y-6 max-w-3xl mx-auto select-none">
      <div className="flex items-center justify-between border-b border-neutral-800/80 pb-4">
        <div>
          <h1 className="text-lg font-bold text-white">系统设置</h1>
          <p className="text-xs text-neutral-500 mt-1">管理应用状态、网络代理与本地缓存</p>
        </div>
        {savedNote && (
          <span className="flex items-center gap-1 text-xs text-emerald-400">
            <Check className="h-3.5 w-3.5" />
            配置已保存（部分设置重启生效）
          </span>
        )}
      </div>

      {/* Backend Diagnostics */}
      <section className="rounded-xl border border-neutral-800/80 bg-neutral-900/50 p-5 space-y-4">
        <div className="flex items-center gap-2 text-sm font-semibold text-neutral-200">
          <Radio className="h-4 w-4 text-rose-500" />
          <span>服务状态与诊断</span>
        </div>

        <div className="grid grid-cols-2 gap-3 text-xs">
          <div className="rounded-lg bg-neutral-950/60 p-3 border border-neutral-800/50">
            <span className="text-neutral-500 block mb-1">Linux MPRIS 桥接</span>
            <span className="font-medium text-neutral-200">
              {status?.mpris === 'ready' ? (
                <span className="text-emerald-400">就绪 (D-Bus Active)</span>
              ) : status?.mpris === 'unavailable' ? (
                <span className="text-amber-500">不可用 (运行环境未连接 D-Bus)</span>
              ) : (
                status?.mpris
              )}
            </span>
          </div>

          <div className="rounded-lg bg-neutral-950/60 p-3 border border-neutral-800/50">
            <span className="text-neutral-500 block mb-1">存储持久化模式</span>
            <span className="font-medium text-neutral-200">
              {status?.persistence === 'ready' ? (
                <span className="text-emerald-400">正常 (Ready)</span>
              ) : (
                <span className="text-rose-400">降级 (Degraded)</span>
              )}
            </span>
          </div>
        </div>
      </section>

      {/* Cache Management */}
      <section className="rounded-xl border border-neutral-800/80 bg-neutral-900/50 p-5 space-y-4">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2 text-sm font-semibold text-neutral-200">
            <Database className="h-4 w-4 text-rose-500" />
            <span>本地歌词缓存</span>
          </div>
          <button
            onClick={handleClearCache}
            className="inline-flex items-center gap-1.5 rounded-lg border border-neutral-800 bg-neutral-900 px-3 py-1.5 text-xs text-rose-400 hover:bg-neutral-800 hover:text-rose-300 transition-colors"
          >
            <Trash2 className="h-3.5 w-3.5" />
            <span>清理歌词缓存</span>
          </button>
        </div>

        <div className="text-xs text-neutral-400 space-y-1">
          <p>
            当前已缓存{' '}
            <strong className="text-neutral-200">{cacheStats?.entries ?? 0}</strong>{' '}
            首歌词，占用磁盘空间{' '}
            <strong className="text-neutral-200">
              {cacheStats ? formatBytes(cacheStats.bytes) : '0 B'}
            </strong>{' '}
            (上限: {cacheStats ? formatBytes(cacheStats.limitBytes) : '32 MB'})。
          </p>
          <p className="text-[11px] text-neutral-500">
            公开歌词默认保留 7 天，按最旧写入时间淘汰。清空后重新播放将重新从云端加载。
          </p>
        </div>
      </section>

      {/* Network Proxy */}
      <section className="rounded-xl border border-neutral-800/80 bg-neutral-900/50 p-5 space-y-4">
        <div className="flex items-center gap-2 text-sm font-semibold text-neutral-200">
          <Globe className="h-4 w-4 text-rose-500" />
          <span>网络代理设置</span>
        </div>

        <div className="space-y-3 text-xs">
          <div className="flex items-center gap-4">
            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="radio"
                name="proxyMode"
                value="system"
                checked={config.proxy.mode === 'system'}
                onChange={() => handleSaveConfig({ ...config, proxy: { mode: 'system' } })}
                className="accent-rose-500"
              />
              <span>跟随系统代理</span>
            </label>

            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="radio"
                name="proxyMode"
                value="direct"
                checked={config.proxy.mode === 'direct'}
                onChange={() => handleSaveConfig({ ...config, proxy: { mode: 'direct' } })}
                className="accent-rose-500"
              />
              <span>直连（绕过代理）</span>
            </label>

            <label className="flex items-center gap-2 cursor-pointer">
              <input
                type="radio"
                name="proxyMode"
                value="http"
                checked={config.proxy.mode === 'http'}
                onChange={() =>
                  handleSaveConfig({
                    ...config,
                    proxy: { mode: 'http', url: proxyUrl || 'http://127.0.0.1:7890' },
                  })
                }
                className="accent-rose-500"
              />
              <span>显式 HTTP 代理</span>
            </label>
          </div>

          {config.proxy.mode === 'http' && (
            <div className="flex items-center gap-2 pt-2">
              <input
                type="text"
                value={proxyUrl}
                onChange={(e) => setProxyUrl(e.target.value)}
                placeholder="http://127.0.0.1:7890"
                className="h-8 flex-1 rounded-lg border border-neutral-800 bg-neutral-950 px-3 text-xs text-neutral-100 placeholder-neutral-600 focus:border-rose-500 focus:outline-none"
              />
              <button
                onClick={() =>
                  handleSaveConfig({
                    ...config,
                    proxy: { mode: 'http', url: proxyUrl.trim() },
                  })
                }
                className="h-8 rounded-lg bg-rose-600 px-3 text-xs font-medium text-white hover:bg-rose-500"
              >
                应用
              </button>
            </div>
          )}

          <div className="flex items-center gap-1 text-[11px] text-neutral-500">
            <Info className="h-3.5 w-3.5 shrink-0" />
            <span>代理模式同时作用于 Reqwest 请求层与 GStreamer 音频流，重启应用后生效。</span>
          </div>
        </div>
      </section>

      {/* Playback Settings */}
      <section className="rounded-xl border border-neutral-800/80 bg-neutral-900/50 p-5 space-y-5">
        <div className="flex items-center gap-2 text-sm font-semibold text-neutral-200">
          <Sliders className="h-4 w-4 text-rose-500" />
          <span>播放偏好与行为</span>
        </div>

        {/* Default Sound Quality Preference */}
        <div className="space-y-2 text-xs border-b border-neutral-800/60 pb-4">
          <div className="flex items-center justify-between">
            <div>
              <span className="font-medium text-neutral-200 block">默认播放音质</span>
              <span className="text-[11px] text-neutral-500">
                新曲目播放时优先请求的音频规格（非 VIP 或音源缺失时会自动降级播放）
              </span>
            </div>
            <select
              value={config.defaultQuality || 'exhigh'}
              onChange={(e) => {
                const q = e.target.value as SoundQuality;
                handleSaveConfig({ ...config, defaultQuality: q });
                playerActions.setQuality(q);
              }}
              className="rounded-lg border border-neutral-700 bg-neutral-950 px-3 py-1.5 text-xs text-neutral-200 focus:border-rose-500 focus:outline-none"
            >
              <option value="standard">标准 (128 kbps MP3)</option>
              <option value="higher">较高 (192 kbps MP3)</option>
              <option value="exhigh">极高 (320 kbps MP3 - 推荐)</option>
              <option value="lossless">无损 (FLAC / 16bit SQ)</option>
              <option value="hires">Hi-Res (高解析母带 / 24bit)</option>
            </select>
          </div>
        </div>

        <div className="flex items-center justify-between text-xs">
          <div>
            <span className="font-medium text-neutral-200 block">启动时自动恢复播放队列</span>
            <span className="text-[11px] text-neutral-500">
              下次打开客户端时自动还原上次关闭前的播放曲目列表
            </span>
          </div>
          <input
            type="checkbox"
            checked={config.restoreQueue}
            onChange={(e) =>
              handleSaveConfig({ ...config, restoreQueue: e.target.checked })
            }
            className="h-4 w-4 rounded accent-rose-500 cursor-pointer"
          />
        </div>
      </section>

      {/* About AkaEase Brand Section */}
      <section className="rounded-2xl border border-neutral-800/80 bg-neutral-900/40 p-6 flex flex-col items-center justify-center text-center space-y-3">
        <BrandLogo
          variant="full"
          className="h-10 drop-shadow-[0_2px_12px_rgba(249,38,54,0.35)]"
        />
        <div className="space-y-1">
          <p className="text-xs font-semibold text-neutral-200">
            AkaEase Linux 原生音乐客户端 v0.1.0
          </p>
          <p className="text-[11px] text-neutral-500 font-mono">
            Powered by Tauri 2 · GStreamer Engine · Linux MPRIS
          </p>
        </div>
      </section>
    </div>
  );
}
