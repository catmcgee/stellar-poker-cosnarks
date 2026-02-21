"use client";

import { decodeCard } from "@/lib/cards";

interface CardProps {
  value?: number;
  faceDown?: boolean;
  size?: "sm" | "md" | "lg";
}

const SUIT_SYMBOLS: Record<string, string> = {
  hearts: '♥',
  diamonds: '♦',
  clubs: '♣',
  spades: '♠',
};

const SUIT_COLORS: Record<string, string> = {
  hearts: '#e74c3c',
  diamonds: '#e74c3c',
  clubs: '#2c3e50',
  spades: '#2c3e50',
};

/* Pixel card back: dark blue with a small star/S pattern */
function CardBack({ w, h }: { w: number; h: number }) {
  return (
    <div
      className="pixel-border-thin flex items-center justify-center"
      style={{
        width: `${w}px`,
        height: `${h}px`,
        background: 'linear-gradient(180deg, #1a3a5c 0%, #0d2137 100%)',
        position: 'relative',
        overflow: 'hidden',
      }}
    >
      {/* Crosshatch pixel pattern */}
      <div style={{
        position: 'absolute',
        inset: '6px',
        border: '2px solid #2a5a8c',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
      }}>
        <div style={{
          color: '#3498db',
          fontSize: '10px',
          textShadow: '1px 1px 0 #0d2137',
        }}>
          S
        </div>
      </div>
    </div>
  );
}

export function Card({ value, faceDown = false, size = "md" }: CardProps) {
  const dims = {
    sm: { w: 44, h: 62, suitSize: '16px', rankSize: '7px' },
    md: { w: 56, h: 80, suitSize: '22px', rankSize: '9px' },
    lg: { w: 72, h: 100, suitSize: '28px', rankSize: '11px' },
  };
  const d = dims[size];

  if (faceDown || value === undefined) {
    return <CardBack w={d.w} h={d.h} />;
  }

  const card = decodeCard(value);
  const color = card.color === 'red' ? '#e74c3c' : '#2c3e50';

  const suitSymbol = SUIT_SYMBOLS[card.suit] || '♠';

  return (
    <div
      className="pixel-border-white flex flex-col items-center justify-between animate-card-deal"
      style={{
        width: `${d.w}px`,
        height: `${d.h}px`,
        background: '#fefefe',
        padding: '4px',
        imageRendering: 'auto',
      }}
    >
      {/* Top-left rank + suit */}
      <div className="w-full flex flex-col items-start" style={{
        color,
        lineHeight: 1,
        paddingLeft: '2px',
      }}>
        <span style={{ fontSize: d.rankSize }}>{card.rank}</span>
        <span style={{ fontSize: d.rankSize, fontFamily: 'serif' }}>{suitSymbol}</span>
      </div>

      {/* Center suit */}
      <div className="flex items-center justify-center flex-1" style={{
        color,
        fontSize: d.suitSize,
        fontFamily: 'serif',
        lineHeight: 1,
      }}>
        {suitSymbol}
      </div>

      {/* Bottom-right rank + suit (inverted) */}
      <div className="w-full flex flex-col items-end" style={{
        color,
        lineHeight: 1,
        paddingRight: '2px',
        transform: 'rotate(180deg)',
      }}>
        <span style={{ fontSize: d.rankSize }}>{card.rank}</span>
        <span style={{ fontSize: d.rankSize, fontFamily: 'serif' }}>{suitSymbol}</span>
      </div>
    </div>
  );
}
