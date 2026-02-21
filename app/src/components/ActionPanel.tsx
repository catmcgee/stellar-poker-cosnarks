"use client";

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
  onAction,
  onChainConfirmed,
}: ActionPanelProps) {
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
  return (
    <div className="text-center">
      <span
        className="text-[8px]"
        style={{
          color: "#95a5a6",
          textShadow: "1px 1px 0 rgba(0,0,0,0.4)",
        }}
      >
        BETTING ACTIONS ARE CURRENTLY DRIVEN ON-CHAIN; USE THE DEAL/REVEAL BUTTONS ABOVE.
      </span>
    </div>
  );
}
