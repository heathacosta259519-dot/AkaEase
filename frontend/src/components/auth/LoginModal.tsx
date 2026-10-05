import { useState, useEffect, useRef } from 'react';
import QRCode from 'qrcode';
import { 
  X, 
  RefreshCw, 
  CheckCircle2, 
  QrCode, 
  Smartphone, 
  AlertCircle 
} from 'lucide-react';
import { useSessionStore, sessionActions } from '../../stores/sessionStore';

export function LoginModal() {
  const isOpen = useSessionStore((s) => s.isLoginModalOpen);
  const challenge = useSessionStore((s) => s.qrChallenge);
  const loginPhase = useSessionStore((s) => s.loginPhase);
  const loginError = useSessionStore((s) => s.loginError);

  const [qrDataUrl, setQrDataUrl] = useState<string | null>(null);
  const [imageLoading, setImageLoading] = useState(false);
  const imageGenRef = useRef(0);

  useEffect(() => {
    if (!isOpen) {
      setQrDataUrl(null);
      setImageLoading(false);
      return;
    }

    if (
      challenge?.qrUrl &&
      (loginPhase === 'waiting_scan' || loginPhase === 'waiting_confirmation')
    ) {
      const gen = ++imageGenRef.current;
      setImageLoading(true);

      QRCode.toDataURL(challenge.qrUrl, {
        width: 220,
        margin: 2,
        color: {
          dark: '#000000',
          light: '#ffffff',
        },
      })
        .then((url) => {
          if (gen === imageGenRef.current) {
            setQrDataUrl(url);
            setImageLoading(false);
          }
        })
        .catch((err) => {
          if (gen === imageGenRef.current) {
            setImageLoading(false);
            sessionActions.setImageError(err);
          }
        });
    } else if (loginPhase !== 'waiting_scan' && loginPhase !== 'waiting_confirmation') {
      setQrDataUrl(null);
      setImageLoading(false);
    }
  }, [isOpen, challenge?.qrUrl, loginPhase]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm select-none p-4 animate-in fade-in duration-150">
      <div className="relative w-full max-w-sm rounded-2xl border border-neutral-800 bg-neutral-900 p-6 shadow-2xl space-y-6">
        {/* Close Button */}
        <button
          onClick={() => sessionActions.closeLoginModal()}
          className="absolute right-4 top-4 rounded-lg p-1 text-neutral-400 hover:bg-neutral-800 hover:text-white transition-colors"
          title="关闭"
        >
          <X className="h-4 w-4" />
        </button>

        {/* Modal Header */}
        <div className="text-center space-y-1">
          <h2 className="text-base font-bold text-white">扫码登录云音乐账号</h2>
          <p className="text-xs text-neutral-400">
            打开配套手机客户端扫一扫登录
          </p>
        </div>

        {/* QR Code Container */}
        <div className="relative flex h-60 w-60 mx-auto items-center justify-center overflow-hidden rounded-xl bg-white p-3 shadow-inner">
          {/* Authenticated */}
          {loginPhase === 'authenticated' && (
            <div className="flex flex-col items-center justify-center gap-2 text-emerald-600">
              <CheckCircle2 className="h-14 w-14 animate-bounce" />
              <span className="text-xs font-semibold">登录成功</span>
            </div>
          )}

          {/* Fetching Challenge */}
          {loginPhase === 'fetching' && (
            <div className="flex flex-col items-center justify-center text-neutral-600 gap-2 text-xs">
              <RefreshCw className="h-7 w-7 animate-spin text-rose-600" />
              <span>正在获取安全二维码...</span>
            </div>
          )}

          {/* Failed / Error */}
          {loginPhase === 'failed' && (
            <div className="flex flex-col items-center justify-center gap-3 p-4 text-center text-neutral-700">
              <AlertCircle className="h-10 w-10 text-rose-500" />
              <div className="space-y-1">
                <span className="text-xs font-medium text-neutral-800 block">
                  {loginError?.message || '获取二维码失败'}
                </span>
                <span className="text-[10px] text-neutral-500 block">
                  错误代码: {loginError?.code || 'unknown'}
                </span>
              </div>
              <button
                onClick={() => sessionActions.startLogin()}
                className="inline-flex items-center gap-1.5 rounded-lg bg-rose-600 px-3.5 py-1.5 text-xs font-medium text-white shadow hover:bg-rose-500 transition-colors"
              >
                <RefreshCw className="h-3 w-3" />
                <span>重新获取</span>
              </button>
            </div>
          )}

          {/* Expired */}
          {loginPhase === 'expired' && (
            <div className="flex flex-col items-center justify-center gap-3 p-4 text-center text-neutral-700">
              <div className="relative flex items-center justify-center">
                {qrDataUrl && (
                  <img
                    src={qrDataUrl}
                    alt="Expired QR"
                    className="h-44 w-44 rounded opacity-15 blur-xs"
                  />
                )}
                <div className="absolute inset-0 flex flex-col items-center justify-center gap-2">
                  <span className="text-xs font-medium text-neutral-800">二维码已失效</span>
                  <button
                    onClick={() => sessionActions.startLogin()}
                    className="inline-flex items-center gap-1.5 rounded-lg bg-rose-600 px-3.5 py-1.5 text-xs font-medium text-white shadow hover:bg-rose-500 transition-colors"
                  >
                    <RefreshCw className="h-3 w-3" />
                    <span>刷新二维码</span>
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* Waiting Scan or Waiting Confirmation */}
          {(loginPhase === 'waiting_scan' || loginPhase === 'waiting_confirmation') && (
            <div className="relative flex items-center justify-center h-full w-full">
              {imageLoading && !qrDataUrl ? (
                <div className="flex flex-col items-center justify-center text-neutral-500 gap-2 text-xs">
                  <RefreshCw className="h-6 w-6 animate-spin text-rose-600" />
                  <span>渲染二维码中...</span>
                </div>
              ) : qrDataUrl ? (
                <>
                  <img
                    src={qrDataUrl}
                    alt="Login QR Code"
                    className="h-52 w-52 rounded"
                  />

                  {loginPhase === 'waiting_confirmation' && (
                    <div className="absolute inset-0 flex flex-col items-center justify-center rounded-lg bg-black/65 text-white backdrop-blur-[1px] gap-2 p-4 text-center animate-in fade-in duration-150">
                      <Smartphone className="h-10 w-10 text-rose-400 animate-pulse" />
                      <span className="text-xs font-medium">请在手机端点击确认</span>
                    </div>
                  )}
                </>
              ) : null}
            </div>
          )}
        </div>

        {/* Footer Hint */}
        <div className="flex items-center justify-center gap-2 text-xs text-neutral-400">
          <QrCode className="h-4 w-4 text-rose-500" />
          <span>
            {loginPhase === 'waiting_confirmation'
              ? '手机端已扫描，等待点击确认'
              : loginPhase === 'expired'
              ? '二维码已超时'
              : loginPhase === 'failed'
              ? '登录遇到异常'
              : '支持移动端 App 扫码'}
          </span>
        </div>
      </div>
    </div>
  );
}
