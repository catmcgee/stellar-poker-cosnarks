"use client";

import { Card } from "./Card";
import { PixelCat, opponentSprite } from "./PixelCat";
import { PixelChip } from "./PixelChip";
import type { Player } from "@/lib/game-state";

interface PlayerSeatProps {
  player: Player;
  isCurrentTurn: boolean;
  isDealer: boolean;
  isUser: boolean;
  isWinner?: boolean;
  isBot?: boolean;
  labelOverride?: string;
}

export function PlayerSeat({
  player,
  isCurrentTurn,
  isDealer,
  isUser,
  isWinner = false,
  isBot = false,
  labelOverride,
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
          fontSize: '9px',
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
          fontSize: "9px",
          color: "#f1c40f",
          textShadow: "1px 1px 0 rgba(0,0,0,0.6)",
          marginBottom: '2px',
        }}>
          ★ WINNER ★
        </div>
      )}

      {/* Label */}
      <div className="text-[9px] mb-1" style={{
        color: isUser ? '#f1c40f' : '#95a5a6',
        textShadow: '1px 1px 0 rgba(0,0,0,0.5)',
      }}>
        {labelOverride ?? (isUser ? "— YOU —" : isBot ? "— AI BOT —" : `${player.address.slice(0, 4)}...${player.address.slice(-4)}`)}
        {isDealer && <span style={{ color: '#f1c40f', marginLeft: '4px' }}>[D]</span>}
      </div>

      {/* Avatar */}
      <div style={{ marginBottom: '4px' }}>
        {isBot ? (
          <img
            src="/cat_sprites/bot.png"
            alt="AI Bot"
            width={48}
            height={48}
            style={{ imageRendering: "pixelated" }}
          />
        ) : (
          <PixelCat
            sprite={sprite}
            size={isUser ? 72 : 48}
            isUser={isUser}
          />
        )}
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
      <div className="flex items-center gap-1 mt-1">
        <PixelChip color={player.stack >= 5000 ? "gold" : player.stack >= 500 ? "blue" : "red"} size={isUser ? 2 : 1} />
        <span className="text-[10px]" style={{
          color: '#27ae60',
          textShadow: '1px 1px 0 rgba(0,0,0,0.4)',
        }}>
          {player.stack.toLocaleString()} CHIPS
        </span>
      </div>

      {/* Bet */}
      {player.betThisRound > 0 && (
        <div
          className="flex items-center gap-1"
          style={{
            animation: "chipBounce 0.4s ease-out",
          }}
        >
          <PixelChip color="gold" size={1} />
          <span className="text-[9px]" style={{ color: '#f1c40f' }}>
            BET: {player.betThisRound.toLocaleString()}
          </span>
        </div>
      )}

      {/* Status tags */}
      {player.folded && (
        <div className="text-[9px]" style={{ color: '#e74c3c' }}>FOLDED</div>
      )}
      {player.allIn && (
        <div className="text-[9px]" style={{
          color: '#e67e22',
          animation: 'textPulse 0.8s ease-in-out infinite',
        }}>
          ALL IN!
        </div>
      )}
    </div>
  );
}
