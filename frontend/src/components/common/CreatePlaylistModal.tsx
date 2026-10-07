import { useState, useRef, useEffect } from 'react';
import { X, Plus, Loader2 } from 'lucide-react';
import { sessionActions } from '../../stores/sessionStore';
import { viewActions } from '../../stores/viewStore';

interface CreatePlaylistModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export function CreatePlaylistModal({ isOpen, onClose }: CreatePlaylistModalProps) {
  const [name, setName] = useState('');
  const [privacy, setPrivacy] = useState(0); // 0: public, 10: private
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (isOpen) {
      setName('');
      setError(null);
      setLoading(false);
      setTimeout(() => inputRef.current?.focus(), 50);
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const handleSubmit = async (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    const trimmed = name.trim();
    if (!trimmed) {
      setError('请输入歌单名称');
      return;
    }
    if (trimmed.length > 40) {
      setError('歌单名称最多 40 个字');
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const created = await sessionActions.createPlaylist(trimmed, privacy);
      onClose();
      viewActions.openPlaylist(created.id);
    } catch (err) {
      setError(err instanceof Error ? err.message : '创建歌单失败，请重试');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4 animate-in fade-in duration-150"
      onClick={onClose}
    >
      <div
        className="relative w-full max-w-md rounded-2xl border border-neutral-800 bg-neutral-900/95 p-6 shadow-2xl transition-all"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Close Button */}
        <button
          onClick={onClose}
          className="absolute right-4 top-4 p-1.5 text-neutral-400 hover:text-white rounded-lg hover:bg-neutral-800 transition-colors cursor-pointer"
        >
          <X className="h-5 w-5" />
        </button>

        {/* Header */}
        <div className="flex items-center gap-2.5 mb-5">
          <div className="flex h-9 w-9 items-center justify-center rounded-xl bg-rose-500/15 text-rose-500">
            <Plus className="h-5 w-5" />
          </div>
          <h3 className="text-lg font-bold text-neutral-100">新建歌单</h3>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-xs font-medium text-neutral-400 mb-1.5">
              歌单标题
            </label>
            <input
              ref={inputRef}
              type="text"
              value={name}
              onChange={(e) => {
                setName(e.target.value);
                if (error) setError(null);
              }}
              placeholder="输入新歌单标题..."
              maxLength={40}
              className="w-full rounded-xl border border-neutral-700/80 bg-neutral-800/80 px-3.5 py-2.5 text-sm text-neutral-100 placeholder-neutral-500 outline-none focus:border-rose-500 focus:ring-1 focus:ring-rose-500 transition-all"
            />
          </div>

          <div className="flex items-center gap-2">
            <input
              type="checkbox"
              id="privacy-checkbox"
              checked={privacy === 10}
              onChange={(e) => setPrivacy(e.target.checked ? 10 : 0)}
              className="h-4 w-4 rounded border-neutral-700 bg-neutral-800 text-rose-500 focus:ring-rose-500 cursor-pointer"
            />
            <label htmlFor="privacy-checkbox" className="text-xs text-neutral-400 select-none cursor-pointer">
              设为隐私歌单（仅自己可见）
            </label>
          </div>

          {error && (
            <div className="text-xs text-rose-400 bg-rose-500/10 border border-rose-500/20 rounded-lg px-3 py-2">
              {error}
            </div>
          )}

          {/* Action Buttons */}
          <div className="flex items-center justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={onClose}
              disabled={loading}
              className="rounded-xl px-4 py-2 text-sm font-medium text-neutral-400 hover:text-neutral-200 hover:bg-neutral-800/80 transition-colors cursor-pointer"
            >
              取消
            </button>
            <button
              type="submit"
              disabled={loading || !name.trim()}
              className="flex items-center gap-2 rounded-xl bg-rose-600 px-5 py-2 text-sm font-medium text-white shadow-lg shadow-rose-600/30 hover:bg-rose-500 disabled:opacity-50 disabled:cursor-not-allowed transition-all cursor-pointer"
            >
              {loading && <Loader2 className="h-4 w-4 animate-spin" />}
              <span>创建</span>
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
