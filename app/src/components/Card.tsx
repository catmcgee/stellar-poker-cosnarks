"use client";

import { decodeCard } from "@/lib/cards";

interface CardProps {
  value?: number;
  faceDown?: boolean;
  size?: "sm" | "md" | "lg";
}

export function Card({ value, faceDown = false, size = "md" }: CardProps) {
  const sizes = {
    sm: { w: "w-12", h: "h-18", text: "text-sm" },
    md: { w: "w-16", h: "h-24", text: "text-base" },
    lg: { w: "w-20", h: "h-30", text: "text-lg" },
  };
  const s = sizes[size];

  if (faceDown || value === undefined) {
    return (
      <div
        className={`${s.w} ${s.h} rounded-lg border-2 border-gray-600 bg-gradient-to-br from-blue-900 to-blue-700 flex items-center justify-center shadow-lg`}
      >
        <div className="text-blue-300 text-2xl font-bold">S</div>
      </div>
    );
  }

  const card = decodeCard(value);

  return (
    <div
      className={`${s.w} ${s.h} rounded-lg border-2 border-gray-300 bg-white flex flex-col items-center justify-center shadow-lg ${s.text}`}
    >
      <span
        className={`font-bold ${card.color === "red" ? "text-red-600" : "text-gray-900"}`}
      >
        {card.rank}
      </span>
      <span
        className={`text-xl ${card.color === "red" ? "text-red-600" : "text-gray-900"}`}
      >
        {card.suitSymbol}
      </span>
    </div>
  );
}
