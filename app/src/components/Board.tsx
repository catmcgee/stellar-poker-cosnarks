"use client";

import { Card } from "./Card";
import { PotChipPile } from "./PixelChip";

interface BoardProps {
  cards: number[];
  pot: number;
}

export function Board({ cards, pot }: BoardProps) {
  return (
    <div className="flex flex-col items-center gap-3">
      {/* Pot display */}
      <div className="flex items-center gap-2">
        <PotChipPile amount={pot} size={2} />
        <span className="text-[12px]" style={{
          color: '#f1c40f',
          textShadow: '1px 1px 0 rgba(0,0,0,0.6)',
          marginLeft: '4px',
        }}>
          POT: {pot.toLocaleString()} CHIPS
        </span>
      </div>

      {/* Community cards */}
      <div className="flex gap-2 items-center">
        {cards.map((card, i) => (
          <div key={i} className="animate-card-deal" style={{ animationDelay: `${i * 0.1}s` }}>
            <Card value={card} size="md" />
          </div>
        ))}
        {/* Empty slots */}
        {Array.from({ length: 5 - cards.length }).map((_, i) => (
          <div
            key={`empty-${i}`}
            style={{
              width: '56px',
              height: '80px',
              border: '3px dashed rgba(139, 105, 20, 0.3)',
              background: 'rgba(0, 0, 0, 0.15)',
            }}
          />
        ))}
      </div>
    </div>
  );
}
