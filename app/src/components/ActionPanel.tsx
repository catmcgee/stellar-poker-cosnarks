"use client";

import { useState } from "react";
import type { GamePhase } from "@/lib/game-state";
import { PixelHeart } from "./PixelCat";

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
      <div className="flex justify-center">
        <button
          onClick={() => onAction("start")}
          className="pixel-btn pixel-btn-green text-[10px]"
        >
          DEAL CARDS
        </button>
      </div>
    );
  }

  if (phase === "showdown" || phase === "settlement") {
    return (
      <div className="flex flex-col items-center gap-3">
        <div className="flex items-center gap-2">
          <span className="text-[8px]" style={{ color: '#95a5a6' }}>
            {phase === "showdown" ? "SHOWDOWN..." : "HAND COMPLETE"}
          </span>
          {onChainConfirmed && (
            <span className="text-[7px] flex items-center gap-1" style={{ color: '#27ae60' }}>
              <PixelHeart size={2} />
              ON-CHAIN
            </span>
          )}
        </div>
        {phase === "settlement" && (
          <button
            onClick={() => onAction("start")}
            className="pixel-btn pixel-btn-green text-[9px]"
          >
            NEW HAND
          </button>
        )}
      </div>
    );
  }

  if (!isMyTurn) {
    return (
      <div className="text-center" style={{
        animation: 'textPulse 1.5s ease-in-out infinite',
      }}>
        <span className="text-[9px]" style={{
          color: '#95a5a6',
          textShadow: '1px 1px 0 rgba(0,0,0,0.4)',
        }}>
          WAITING FOR OPPONENT...
        </span>
      </div>
    );
  }

  const callAmount = currentBet - myBet;
  const canCheck = callAmount === 0;
  const minRaise = currentBet * 2;

  return (
    <div className="flex flex-col gap-3 items-center">
      {/* Action buttons */}
      <div className="flex gap-2 flex-wrap justify-center">
        <button
          onClick={() => onAction("fold")}
          className="pixel-btn pixel-btn-red text-[8px]"
        >
          FOLD
        </button>

        {canCheck ? (
          <button
            onClick={() => onAction("check")}
            className="pixel-btn pixel-btn-blue text-[8px]"
          >
            CHECK
          </button>
        ) : (
          <button
            onClick={() => onAction("call", callAmount)}
            className="pixel-btn pixel-btn-blue text-[8px]"
          >
            CALL {callAmount}
          </button>
        )}

        <button
          onClick={() => onAction("raise", raiseAmount)}
          className="pixel-btn pixel-btn-gold text-[8px]"
          disabled={myStack < minRaise}
          style={{ opacity: myStack < minRaise ? 0.5 : 1 }}
        >
          RAISE {raiseAmount}
        </button>

        <button
          onClick={() => onAction("allin", myStack)}
          className="pixel-btn pixel-btn-orange text-[8px]"
        >
          ALL IN ({myStack})
        </button>
      </div>

      {/* Raise slider */}
      <div className="flex items-center gap-2">
        <span className="text-[7px]" style={{ color: '#7f8c8d' }}>{minRaise}</span>
        <input
          type="range"
          min={minRaise}
          max={myStack}
          value={raiseAmount}
          onChange={(e) => setRaiseAmount(Number(e.target.value))}
          className="w-40"
        />
        <span className="text-[7px]" style={{ color: '#7f8c8d' }}>{myStack}</span>
      </div>
    </div>
  );
}
