"use client";

import { useState } from "react";
import type { GamePhase } from "@/lib/game-state";

interface ActionPanelProps {
  phase: GamePhase;
  isMyTurn: boolean;
  currentBet: number;
  myBet: number;
  myStack: number;
  onAction: (action: string, amount?: number) => void;
  onChainConfirmed?: boolean;
}

export function ActionPanel({
  phase,
  isMyTurn,
  currentBet,
  myBet,
  myStack,
  onAction,
  onChainConfirmed,
}: ActionPanelProps) {
  const [raiseAmount, setRaiseAmount] = useState(currentBet * 2);

  if (phase === "waiting") {
    return (
      <div className="flex gap-3">
        <button
          onClick={() => onAction("start")}
          className="px-6 py-3 bg-green-600 hover:bg-green-500 text-white rounded-lg font-bold transition"
        >
          Start Hand
        </button>
      </div>
    );
  }

  if (phase === "showdown" || phase === "settlement") {
    return (
      <div className="flex flex-col items-center gap-2">
        <div className="flex items-center gap-2 text-gray-400">
          {phase === "showdown" ? "Showdown in progress..." : "Hand complete"}
          {onChainConfirmed && (
            <span className="text-green-400 text-sm" title="Verified on-chain">
              &#10003; On-chain
            </span>
          )}
        </div>
        {phase === "settlement" && (
          <button
            onClick={() => onAction("start")}
            className="px-6 py-2 bg-green-600 hover:bg-green-500 text-white rounded-lg font-bold transition"
          >
            New Hand
          </button>
        )}
      </div>
    );
  }

  if (!isMyTurn) {
    return (
      <div className="text-center text-gray-400 animate-pulse">
        Waiting for opponent...
      </div>
    );
  }

  const callAmount = currentBet - myBet;
  const canCheck = callAmount === 0;
  const minRaise = currentBet * 2;

  return (
    <div className="flex flex-col gap-3">
      <div className="flex gap-3 justify-center">
        <button
          onClick={() => onAction("fold")}
          className="px-5 py-2 bg-red-700 hover:bg-red-600 text-white rounded-lg font-medium transition"
        >
          Fold
        </button>

        {canCheck ? (
          <button
            onClick={() => onAction("check")}
            className="px-5 py-2 bg-blue-700 hover:bg-blue-600 text-white rounded-lg font-medium transition"
          >
            Check
          </button>
        ) : (
          <button
            onClick={() => onAction("call", callAmount)}
            className="px-5 py-2 bg-blue-700 hover:bg-blue-600 text-white rounded-lg font-medium transition"
          >
            Call {callAmount}
          </button>
        )}

        <button
          onClick={() => onAction("raise", raiseAmount)}
          className="px-5 py-2 bg-yellow-600 hover:bg-yellow-500 text-white rounded-lg font-medium transition"
          disabled={myStack < minRaise}
        >
          Raise to {raiseAmount}
        </button>

        <button
          onClick={() => onAction("allin", myStack)}
          className="px-5 py-2 bg-orange-600 hover:bg-orange-500 text-white rounded-lg font-bold transition"
        >
          All In ({myStack})
        </button>
      </div>

      <div className="flex items-center gap-2 justify-center">
        <input
          type="range"
          min={minRaise}
          max={myStack}
          value={raiseAmount}
          onChange={(e) => setRaiseAmount(Number(e.target.value))}
          className="w-48"
        />
        <span className="text-sm text-gray-400 w-16">{raiseAmount}</span>
      </div>
    </div>
  );
}
