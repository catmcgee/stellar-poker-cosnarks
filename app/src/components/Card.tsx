"use client";

import { decodeCard } from "@/lib/cards";

interface CardProps {
  value?: number;
  faceDown?: boolean;
  size?: "sm" | "md" | "lg";
}

const SUIT_COLORS: Record<string, string> = {
  hearts: '#e74c3c',
  diamonds: '#e74c3c',
  clubs: '#2c3e50',
  spades: '#2c3e50',
};

/* Pixel-art suit sprites via box-shadow. Each suit is drawn on a 5x5 "pixel" grid. */
function PixelSuit({ suit, px }: { suit: string; px: number }) {
  const c = SUIT_COLORS[suit] || '#2c3e50';

  const sprites: Record<string, string> = {
    hearts: `
      ${1*px}px ${0}px 0 ${c}, ${3*px}px ${0}px 0 ${c},
      ${0}px ${1*px}px 0 ${c}, ${1*px}px ${1*px}px 0 ${c}, ${2*px}px ${1*px}px 0 ${c}, ${3*px}px ${1*px}px 0 ${c}, ${4*px}px ${1*px}px 0 ${c},
      ${0}px ${2*px}px 0 ${c}, ${1*px}px ${2*px}px 0 ${c}, ${2*px}px ${2*px}px 0 ${c}, ${3*px}px ${2*px}px 0 ${c}, ${4*px}px ${2*px}px 0 ${c},
      ${1*px}px ${3*px}px 0 ${c}, ${2*px}px ${3*px}px 0 ${c}, ${3*px}px ${3*px}px 0 ${c},
      ${2*px}px ${4*px}px 0 ${c}
    `,
    diamonds: `
      ${2*px}px ${0}px 0 ${c},
      ${1*px}px ${1*px}px 0 ${c}, ${2*px}px ${1*px}px 0 ${c}, ${3*px}px ${1*px}px 0 ${c},
      ${0}px ${2*px}px 0 ${c}, ${1*px}px ${2*px}px 0 ${c}, ${2*px}px ${2*px}px 0 ${c}, ${3*px}px ${2*px}px 0 ${c}, ${4*px}px ${2*px}px 0 ${c},
      ${1*px}px ${3*px}px 0 ${c}, ${2*px}px ${3*px}px 0 ${c}, ${3*px}px ${3*px}px 0 ${c},
      ${2*px}px ${4*px}px 0 ${c}
    `,
    clubs: `
      ${2*px}px ${0}px 0 ${c},
      ${0}px ${1*px}px 0 ${c}, ${1*px}px ${1*px}px 0 ${c}, ${2*px}px ${1*px}px 0 ${c}, ${3*px}px ${1*px}px 0 ${c}, ${4*px}px ${1*px}px 0 ${c},
      ${0}px ${2*px}px 0 ${c}, ${1*px}px ${2*px}px 0 ${c}, ${2*px}px ${2*px}px 0 ${c}, ${3*px}px ${2*px}px 0 ${c}, ${4*px}px ${2*px}px 0 ${c},
      ${2*px}px ${3*px}px 0 ${c},
      ${1*px}px ${4*px}px 0 ${c}, ${2*px}px ${4*px}px 0 ${c}, ${3*px}px ${4*px}px 0 ${c}
    `,
    spades: `
      ${2*px}px ${0}px 0 ${c},
      ${1*px}px ${1*px}px 0 ${c}, ${2*px}px ${1*px}px 0 ${c}, ${3*px}px ${1*px}px 0 ${c},
      ${0}px ${2*px}px 0 ${c}, ${1*px}px ${2*px}px 0 ${c}, ${2*px}px ${2*px}px 0 ${c}, ${3*px}px ${2*px}px 0 ${c}, ${4*px}px ${2*px}px 0 ${c},
      ${0}px ${3*px}px 0 ${c}, ${1*px}px ${3*px}px 0 ${c}, ${2*px}px ${3*px}px 0 ${c}, ${3*px}px ${3*px}px 0 ${c}, ${4*px}px ${3*px}px 0 ${c},
      ${1*px}px ${4*px}px 0 ${c}, ${2*px}px ${4*px}px 0 ${c}, ${3*px}px ${4*px}px 0 ${c}
    `,
  };

  return (
    <div style={{
      width: `${px}px`,
      height: `${px}px`,
      background: 'transparent',
      boxShadow: sprites[suit] || sprites.spades,
    }} />
  );
}

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
    sm: { w: 44, h: 62, suitPx: 2, rankSize: '7px' },
    md: { w: 56, h: 80, suitPx: 3, rankSize: '8px' },
    lg: { w: 72, h: 100, suitPx: 3, rankSize: '10px' },
  };
  const d = dims[size];

  if (faceDown || value === undefined) {
    return <CardBack w={d.w} h={d.h} />;
  }

  const card = decodeCard(value);
  const color = card.color === 'red' ? '#e74c3c' : '#2c3e50';

  return (
    <div
      className="pixel-border-white flex flex-col items-center justify-between animate-card-deal"
      style={{
        width: `${d.w}px`,
        height: `${d.h}px`,
        background: '#fefefe',
        padding: '4px',
      }}
    >
      {/* Top-left rank */}
      <div className="w-full flex justify-start" style={{
        color,
        fontSize: d.rankSize,
        lineHeight: 1,
        paddingLeft: '2px',
      }}>
        {card.rank}
      </div>

      {/* Center suit */}
      <div className="flex items-center justify-center flex-1">
        <PixelSuit suit={card.suit} px={d.suitPx} />
      </div>

      {/* Bottom-right rank (inverted) */}
      <div className="w-full flex justify-end" style={{
        color,
        fontSize: d.rankSize,
        lineHeight: 1,
        paddingRight: '2px',
        transform: 'rotate(180deg)',
      }}>
        {card.rank}
      </div>
    </div>
  );
}
