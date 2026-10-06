import logoMark from '../../assets/logo-mark.png';
import logoFull from '../../assets/logo-full.png';

interface BrandLogoProps {
  variant?: 'mark' | 'full';
  glow?: boolean;
  className?: string;
  alt?: string;
}

export function BrandLogo({
  variant = 'mark',
  glow = false,
  className = '',
  alt = 'AkaEase',
}: BrandLogoProps) {
  const src = variant === 'mark' ? logoMark : logoFull;

  return (
    <img
      src={src}
      alt={alt}
      draggable={false}
      className={`select-none object-contain pointer-events-none transition-all duration-300 ${
        glow ? 'drop-shadow-[0_0_12px_rgba(249,38,54,0.4)]' : ''
      } ${className}`}
    />
  );
}
