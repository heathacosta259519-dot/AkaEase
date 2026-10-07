import React, { useState, useRef, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { Check, Sparkles, Disc, AlertCircle } from 'lucide-react';
import { usePlayerStore, playerActions } from '../../stores/playerStore';
import type { SoundQuality } from '../../types/backend';

interface QualityOption {
  key: SoundQuality;
  label: string;
  shortLabel: string;
  badge?: string;
  description: string;
}

const QUALITY_OPTIONS: QualityOption[] = [
  {
    key: 'standard',
    label: '标准品质',
    shortLabel: '标准',
    description: '128 kbps · MP3',
  },
  {
    key: 'higher',
    label: '较高品质',
    shortLabel: '较高',
    description: '192 kbps · MP3',
  },
  {
    key: 'exhigh',
    label: '极高品质',
    shortLabel: '极高',
    badge: 'HQ',
    description: '320 kbps · MP3',
  },
  {
    key: 'lossless',
    label: '无损品质',
    shortLabel: '无损',
    badge: 'SQ',
    description: 'FLAC · 16bit / 44.1kHz',
  },
  {
    key: 'hires',
    label: 'Hi-Res 高解析',
    shortLabel: 'Hi-Res',
    badge: 'Hi-Res',
    description: 'FLAC · 24bit / 96kHz+',
  },
];

interface SoundQualitySelectorProps {
  compact?: boolean;
  className?: string;
}

export function SoundQualitySelector({ compact = false, className = '' }: SoundQualitySelectorProps) {
  const snapshot = usePlayerStore((s) => s.snapshot);
  const [isOpen, setIsOpen] = useState(false);
  const [menuPosition, setMenuPosition] = useState<{ top?: number; bottom?: number; left: number }>({ left: 0 });
  const buttonRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);

  const targetQuality = snapshot?.targetQuality ?? 'exhigh';
  const actualQuality = snapshot?.actualQuality;
  const actualBitrate = snapshot?.actualBitrate;
  const format = snapshot?.format;

  // 判断是否发生自动降级（实际音质低于目标音质）
  const qualityRanks: Record<SoundQuality, number> = {
    standard: 1,
    higher: 2,
    exhigh: 3,
    lossless: 4,
    hires: 5,
  };
  const isDegraded = actualQuality && qualityRanks[actualQuality] < qualityRanks[targetQuality];

  const currentOption = QUALITY_OPTIONS.find((q) => q.key === (actualQuality || targetQuality)) || QUALITY_OPTIONS[2];

  // 计算浮层弹出坐标
  const updatePosition = () => {
    if (!buttonRef.current) return;
    const rect = buttonRef.current.getBoundingClientRect();
    const menuWidth = 220;
    const menuHeight = 240;

    let left = rect.left + rect.width / 2 - menuWidth / 2;
    // 限制在视口内
    if (left < 12) left = 12;
    if (left + menuWidth > window.innerWidth - 12) {
      left = window.innerWidth - menuWidth - 12;
    }

    const spaceBelow = window.innerHeight - rect.bottom;
    if (spaceBelow < menuHeight && rect.top > menuHeight) {
      // 空间不够向上展开
      setMenuPosition({
        bottom: window.innerHeight - rect.top + 6,
        left,
      });
    } else {
      // 向下展开
      setMenuPosition({
        top: rect.bottom + 6,
        left,
      });
    }
  };

  const handleToggle = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (!isOpen) {
      updatePosition();
    }
    setIsOpen(!isOpen);
  };

  const handleSelect = (quality: SoundQuality) => {
    playerActions.setQuality(quality);
    setIsOpen(false);
  };

  // 点击外部、按 ESC 或窗口滚动收起
  useEffect(() => {
    if (!isOpen) return;

    const handlePointerDown = (e: MouseEvent) => {
      if (
        menuRef.current?.contains(e.target as Node) ||
        buttonRef.current?.contains(e.target as Node)
      ) {
        return;
      }
      setIsOpen(false);
    };

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setIsOpen(false);
      }
    };

    const handleScroll = () => {
      setIsOpen(false);
    };

    window.addEventListener('pointerdown', handlePointerDown);
    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('scroll', handleScroll, true);
    window.addEventListener('resize', handleScroll);

    return () => {
      window.removeEventListener('pointerdown', handlePointerDown);
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('scroll', handleScroll, true);
      window.removeEventListener('resize', handleScroll);
    };
  }, [isOpen]);

  // 渲染触发胶囊按钮
  const renderTrigger = () => {
    const label = currentOption.shortLabel;
    const isVipQuality = targetQuality === 'lossless' || targetQuality === 'hires';

    return (
      <button
        ref={buttonRef}
        onClick={handleToggle}
        title={
          isDegraded
            ? `期望音质: ${targetQuality}，当前已自动降级为: ${actualQuality}`
            : `当前音质: ${label}${actualBitrate ? ` (${Math.round(actualBitrate / 1000)}k ${format || ''})` : ''}`
        }
        className={`group relative flex items-center gap-1.5 rounded-full px-2 py-0.5 text-xs font-medium transition-all press-feedback-sm ${
          isDegraded
            ? 'border border-amber-500/40 bg-amber-500/10 text-amber-300 hover:bg-amber-500/20'
            : isVipQuality
            ? 'border border-rose-500/30 bg-rose-500/10 text-rose-300 hover:bg-rose-500/20'
            : 'border border-neutral-700/60 bg-neutral-800/80 text-neutral-300 hover:border-neutral-600 hover:text-white'
        } ${className}`}
      >
        {isDegraded ? (
          <AlertCircle className="h-3 w-3 text-amber-400" />
        ) : isVipQuality ? (
          <Sparkles className="h-3 w-3 text-rose-400" />
        ) : (
          <Disc className="h-3 w-3 text-neutral-400 group-hover:text-neutral-200" />
        )}
        <span>{label}</span>
        {format && !compact && (
          <span className="text-[10px] uppercase opacity-60">{format}</span>
        )}
      </button>
    );
  };

  return (
    <>
      {renderTrigger()}

      {isOpen &&
        createPortal(
          <div
            ref={menuRef}
            style={{
              position: 'fixed',
              top: menuPosition.top,
              bottom: menuPosition.bottom,
              left: menuPosition.left,
              zIndex: 9999,
            }}
            className="w-56 rounded-xl border border-neutral-800 bg-neutral-900/95 p-1.5 shadow-2xl backdrop-blur-md animate-in fade-in-0 zoom-in-95 duration-100 select-none"
          >
            <div className="px-2.5 py-1.5 text-[11px] font-semibold text-neutral-400 border-b border-neutral-800/80 mb-1 flex items-center justify-between">
              <span>选择播放音质</span>
              {actualBitrate && (
                <span className="text-neutral-500 font-mono text-[10px]">
                  {Math.round(actualBitrate / 1000)} kbps
                </span>
              )}
            </div>

            {isDegraded && (
              <div className="mx-1 mb-1.5 flex items-center gap-1.5 rounded-lg bg-amber-500/10 p-2 text-[11px] text-amber-300 border border-amber-500/20">
                <AlertCircle className="h-3.5 w-3.5 shrink-0 text-amber-400" />
                <span>该歌曲或账号权限暂无所选音质，已自动按最佳档位播放。</span>
              </div>
            )}

            <div className="flex flex-col gap-0.5">
              {QUALITY_OPTIONS.map((opt) => {
                const isSelected = targetQuality === opt.key;
                const isActual = actualQuality === opt.key;

                return (
                  <button
                    key={opt.key}
                    onClick={() => handleSelect(opt.key)}
                    className={`flex w-full items-center justify-between rounded-lg px-2.5 py-2 text-left transition-colors ${
                      isSelected
                        ? 'bg-rose-500/15 text-rose-300'
                        : 'text-neutral-200 hover:bg-neutral-800/80'
                    }`}
                  >
                    <div className="flex flex-col">
                      <div className="flex items-center gap-1.5">
                        <span className="text-xs font-medium">{opt.label}</span>
                        {opt.badge && (
                          <span
                            className={`rounded px-1 py-0.2 text-[9px] font-bold ${
                              opt.badge === 'Hi-Res'
                                ? 'bg-amber-500/20 text-amber-300'
                                : 'bg-rose-500/20 text-rose-300'
                            }`}
                          >
                            {opt.badge}
                          </span>
                        )}
                        {isDegraded && isActual && (
                          <span className="text-[10px] text-amber-400 font-normal">
                            (当前档位)
                          </span>
                        )}
                      </div>
                      <span className="text-[11px] text-neutral-400">{opt.description}</span>
                    </div>

                    {isSelected && <Check className="h-4 w-4 shrink-0 text-rose-400" />}
                  </button>
                );
              })}
            </div>
          </div>,
          document.body
        )}
    </>
  );
}
