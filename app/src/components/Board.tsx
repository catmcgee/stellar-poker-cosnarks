"use client";

import { Card } from "./Card";

interface BoardProps {
  cards: number[];
  pot: number;
}

export function Board({ cards, pot }: BoardProps) {
  return (
    <div className="flex flex-col items-center gap-4">
      <div className="text-lg font-bold text-yellow-400">
        Pot: {pot.toLocaleString()} XLM
      </div>

      <div className="flex gap-2">
        {cards.map((card, i) => (
          <Card key={i} value={card} size="md" />
        ))}
        {/* Empty slots for remaining cards */}
        {Array.from({ length: 5 - cards.length }).map((_, i) => (
          <div
            key={`empty-${i}`}
            className="w-16 h-24 rounded-lg border-2 border-dashed border-gray-600 bg-gray-800/40"
          />
        ))}
      </div>
    </div>
  );
}
