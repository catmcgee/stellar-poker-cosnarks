"use client";

import { Card } from "./Card";
import { PixelCat, PixelHeart } from "./PixelCat";
import type { Player } from "@/lib/game-state";

interface PlayerSeatProps {
  player: Player;
  isCurrentTurn: boolean;
  isDealer: boolean;
  isUser: boolean;
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
}: PlayerSeatProps) {
  const catConfig = SEAT_CATS[player.seat % SEAT_CATS.length];

  return (
    <div
      className="pixel-border-thin relative flex flex-col items-center gap-2 p-3"
      style={{
        background: player.folded
          ? 'rgba(20, 12, 8, 0.5)'
          : 'var(--ui-panel)',
        opacity: player.folded ? 0.6 : 1,
        minWidth: '130px',
        borderColor: isCurrentTurn ? '#f1c40f' : undefined,
      }}
    >
      {/* Dealer chip */}
      {isDealer && (
        <div className="absolute -top-3 -left-3 z-10" style={{
          width: '20px',
          height: '20px',
          background: '#f1c40f',
          border: '2px solid #b7950b',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          fontSize: '7px',
          color: '#3d2400',
          boxShadow: '0 2px 0 0 #8a7000',
        }}>
          D
        </div>
      )}

      {/* Turn indicator */}
      {isCurrentTurn && !player.folded && (
        <div className="absolute -top-2 left-1/2 -translate-x-1/2" style={{
          animation: 'textPulse 1s ease-in-out infinite',
          fontSize: '6px',
          color: '#f1c40f',
          textShadow: '1px 1px 0 rgba(0,0,0,0.6)',
          whiteSpace: 'nowrap',
        }}>
          YOUR TURN
        </div>
      )}

      {/* Cat avatar + cards row */}
      <div className="flex items-end gap-2">
        {/* Cat avatar */}
        <div className="flex-shrink-0">
          <PixelCat
            variant={catConfig.variant}
            size={isUser ? 5 : 4}
            flipped={catConfig.flipped}
          />
        </div>

        {/* Cards */}
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
      </div>

      {/* Player info */}
      <div className="text-center w-full">
        <div className="text-[7px] truncate" style={{
          color: isUser ? '#f1c40f' : '#bdc3c7',
          maxWidth: '120px',
        }}>
          {isUser ? "YOU" : `${player.address.slice(0, 6)}...`}
        </div>

        {/* Stack with pixel coin icon */}
        <div className="flex items-center justify-center gap-1 mt-1">
          <span className="text-[8px]" style={{
            color: '#27ae60',
            textShadow: '1px 1px 0 rgba(0,0,0,0.4)',
          }}>
            {player.stack.toLocaleString()} XLM
          </span>
        </div>

        {/* Bet */}
        {player.betThisRound > 0 && (
          <div className="text-[7px] mt-0.5" style={{ color: '#f1c40f' }}>
            BET: {player.betThisRound}
          </div>
        )}

        {/* Status tags */}
        {player.folded && (
          <div className="text-[7px] mt-0.5" style={{ color: '#e74c3c' }}>FOLDED</div>
        )}
        {player.allIn && (
          <div className="text-[7px] mt-0.5" style={{
            color: '#e67e22',
            animation: 'textPulse 0.8s ease-in-out infinite',
          }}>
            ALL IN!
          </div>
        )}
      </div>
    </div>
  );
}
