"use client";

import { Card } from "./Card";
import { PixelCat, opponentSprite } from "./PixelCat";
import { PixelChipStack } from "./PixelChip";
import type { Player } from "@/lib/game-state";

interface PlayerSeatProps {
  player: Player;
  isCurrentTurn: boolean;
  isDealer: boolean;
  isUser: boolean;
  isWinner?: boolean;
}

export function PlayerSeat({
  player,
  isCurrentTurn,
  isDealer,
  isUser,
  isWinner = false,
}: PlayerSeatProps) {
  const sprite = isUser ? 18 : opponentSprite(player.seat);
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
          sprite={sprite}
          size={isUser ? 72 : 48}
          isUser={isUser}
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

      {/* Stack with chip visuals */}
      <div className="flex items-center gap-2 mt-1">
        <PixelChipStack amount={player.stack} size={isUser ? 2 : 1} />
        <span className="text-[8px]" style={{
          color: '#27ae60',
          textShadow: '1px 1px 0 rgba(0,0,0,0.4)',
        }}>
          {player.stack.toLocaleString()} XLM
        </span>
      </div>

      {/* Bet with chip visuals */}
      {player.betThisRound > 0 && (
        <div
          className="flex items-center gap-1"
          style={{
            animation: "chipBounce 0.4s ease-out",
          }}
        >
          <PixelChipStack amount={player.betThisRound} size={1} />
          <span className="text-[7px]" style={{ color: '#f1c40f' }}>
            BET: {player.betThisRound}
          </span>
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
