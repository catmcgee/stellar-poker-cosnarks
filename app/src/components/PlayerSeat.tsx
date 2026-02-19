"use client";

import { Card } from "./Card";
import type { Player } from "@/lib/game-state";

interface PlayerSeatProps {
  player: Player;
  isCurrentTurn: boolean;
  isDealer: boolean;
  isUser: boolean;
}

export function PlayerSeat({
  player,
  isCurrentTurn,
  isDealer,
  isUser,
}: PlayerSeatProps) {
  const borderColor = isCurrentTurn
    ? "border-yellow-400"
    : player.folded
      ? "border-gray-600"
      : "border-green-700";

  return (
    <div
      className={`relative flex flex-col items-center gap-2 p-3 rounded-xl border-2 ${borderColor} ${
        player.folded ? "opacity-50" : ""
      } bg-gray-800/80`}
    >
      {isDealer && (
        <div className="absolute -top-2 -left-2 w-6 h-6 rounded-full bg-white text-black text-xs font-bold flex items-center justify-center">
          D
        </div>
      )}

      <div className="flex gap-1">
        {player.cards ? (
          <>
            <Card value={player.cards[0]} size="sm" faceDown={!isUser} />
            <Card value={player.cards[1]} size="sm" faceDown={!isUser} />
          </>
        ) : (
          <>
            <Card faceDown size="sm" />
            <Card faceDown size="sm" />
          </>
        )}
      </div>

      <div className="text-center">
        <div className="text-sm text-gray-300 truncate max-w-[120px]">
          {isUser ? "You" : `${player.address.slice(0, 6)}...`}
        </div>
        <div className="text-sm font-mono text-green-400">
          {player.stack.toLocaleString()} XLM
        </div>
        {player.betThisRound > 0 && (
          <div className="text-xs text-yellow-400">
            Bet: {player.betThisRound}
          </div>
        )}
        {player.folded && (
          <div className="text-xs text-red-400">Folded</div>
        )}
        {player.allIn && (
          <div className="text-xs text-orange-400 font-bold">ALL IN</div>
        )}
      </div>

      {isCurrentTurn && !player.folded && (
        <div className="absolute -bottom-1 left-1/2 -translate-x-1/2 w-2 h-2 rounded-full bg-yellow-400 animate-pulse" />
      )}
    </div>
  );
}
