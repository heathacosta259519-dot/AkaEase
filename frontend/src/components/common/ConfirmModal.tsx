import { Loader2, AlertTriangle } from 'lucide-react';

interface ConfirmModalProps {
  isOpen: boolean;
  title: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  danger?: boolean;
  loading?: boolean;
  onConfirm: () => void;
  onClose: () => void;
}

export function ConfirmModal({
  isOpen,
  title,
  message,
  confirmText = '确定',
  cancelText = '取消',
  danger = false,
  loading = false,
  onConfirm,
  onClose,
}: ConfirmModalProps) {
  if (!isOpen) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm p-4 animate-in fade-in duration-150"
      onClick={onClose}
    >
      <div
        className="relative w-full max-w-sm rounded-2xl border border-neutral-800 bg-neutral-900/95 p-6 shadow-2xl transition-all"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-3 mb-3">
          <div
            className={`flex h-10 w-10 shrink-0 items-center justify-center rounded-xl ${
              danger ? 'bg-rose-500/15 text-rose-500' : 'bg-neutral-800 text-neutral-300'
            }`}
          >
            <AlertTriangle className="h-5 w-5" />
          </div>
          <h3 className="text-base font-bold text-neutral-100">{title}</h3>
        </div>

        <p className="text-xs text-neutral-400 leading-relaxed mb-6 pl-0.5">
          {message}
        </p>

        <div className="flex items-center justify-end gap-3">
          <button
            type="button"
            onClick={onClose}
            disabled={loading}
            className="rounded-xl px-4 py-2 text-xs font-medium text-neutral-400 hover:text-neutral-200 hover:bg-neutral-800/80 transition-colors cursor-pointer"
          >
            {cancelText}
          </button>
          <button
            type="button"
            onClick={onConfirm}
            disabled={loading}
            className={`flex items-center gap-2 rounded-xl px-4 py-2 text-xs font-semibold text-white transition-all cursor-pointer ${
              danger
                ? 'bg-rose-600 hover:bg-rose-500 shadow-lg shadow-rose-600/30'
                : 'bg-neutral-700 hover:bg-neutral-600'
            } disabled:opacity-50`}
          >
            {loading && <Loader2 className="h-3.5 w-3.5 animate-spin" />}
            <span>{confirmText}</span>
          </button>
        </div>
      </div>
    </div>
  );
}
