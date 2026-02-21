"use client";

import { Card } from "./Card";
import { PixelCat, PixelHeart } from "./PixelCat";
import type { Player } from "@/lib/game-state";

interface PlayerSeatProps {
  player: Player;
  isCurrentTurn: boolean;
  isDealer: boolean;
  isUser: boolean;
  isWinner?: boolean;
}

const SEAT_CATS: Array<{ variant: "grey" | "orange" | "black"; flipped?: boolean }> = [
  { variant: "orange" },
  { variant: "grey", flipped: true },
  { variant: "black" },
];

export function PlayerSeat({
  player,
  isCurrentTurn,
  isDealer,
  isUser,
  isWinner = false,
}: PlayerSeatProps) {
  const catConfig = SEAT_CATS[player.seat % SEAT_CATS.length];

  const cardSize = isUser ? "md" : "sm";

  return (
    <div
      className="relative flex flex-col items-center gap-1"
      style={{
        opacity: player.folded ? 0.5 : 1,
      }}
    >
      {/* Turn indicator */}
      {isCurrentTurn && !player.folded && (
        <div style={{
          animation: 'textPulse 1s ease-in-out infinite',
          fontSize: '7px',
          color: '#f1c40f',
          textShadow: '1px 1px 0 rgba(0,0,0,0.6)',
          whiteSpace: 'nowrap',
          marginBottom: '2px',
        }}>
          {isUser ? "▼ YOUR TURN ▼" : "▼ THEIR TURN ▼"}
        </div>
      )}

      {/* Winner badge */}
      {isWinner && (
        <div style={{
          fontSize: "7px",
          color: "#f1c40f",
          textShadow: "1px 1px 0 rgba(0,0,0,0.6)",
          marginBottom: '2px',
        }}>
          ★ WINNER ★
        </div>
      )}

      {/* Label */}
      <div className="text-[7px] mb-1" style={{
        color: isUser ? '#f1c40f' : '#95a5a6',
        textShadow: '1px 1px 0 rgba(0,0,0,0.5)',
      }}>
        {isUser ? "— YOU —" : `${player.address.slice(0, 6)}...`}
        {isDealer && <span style={{ color: '#f1c40f', marginLeft: '4px' }}>[D]</span>}
      </div>

      {/* Cat avatar */}
      <div style={{ marginBottom: '4px' }}>
        <PixelCat
          variant={catConfig.variant}
          size={isUser ? 6 : 4}
          flipped={catConfig.flipped}
        />
      </div>

      {/* Cards */}
      <div className="flex gap-1">
        {player.cards ? (
          <>
            <Card value={player.cards[0]} size={cardSize} faceDown={!isUser} />
            <Card value={player.cards[1]} size={cardSize} faceDown={!isUser} />
          </>
        ) : (
          <>
            <Card faceDown size={cardSize} />
            <Card faceDown size={cardSize} />
          </>
        )}
      </div>

      {/* Stack */}
      <div className="text-[8px] mt-1" style={{
        color: '#27ae60',
        textShadow: '1px 1px 0 rgba(0,0,0,0.4)',
      }}>
        {player.stack.toLocaleString()} XLM
      </div>

      {/* Bet */}
      {player.betThisRound > 0 && (
        <div className="text-[7px]" style={{ color: '#f1c40f' }}>
          BET: {player.betThisRound}
        </div>
      )}

      {/* Status tags */}
      {player.folded && (
        <div className="text-[7px]" style={{ color: '#e74c3c' }}>FOLDED</div>
      )}
      {player.allIn && (
        <div className="text-[7px]" style={{
          color: '#e67e22',
          animation: 'textPulse 0.8s ease-in-out infinite',
        }}>
          ALL IN!
        </div>
      )}
    </div>
  );
}
