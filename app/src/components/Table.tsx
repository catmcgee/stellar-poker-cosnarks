"use client";

import { useState, useEffect, useCallback } from "react";
import { Board } from "./Board";
import { PlayerSeat } from "./PlayerSeat";
import { ActionPanel } from "./ActionPanel";
import type { GameState, Player } from "@/lib/game-state";
import { createInitialState } from "@/lib/game-state";
import * as api from "@/lib/api";

interface TableProps {
  tableId: number;
}

export function Table({ tableId }: TableProps) {
  const [game, setGame] = useState<GameState>(() =>
    createInitialState(tableId)
  );
  const [userAddress] = useState("player_0");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const userPlayer = game.players.find((p) => p.address === userAddress);
  const isMyTurn = game.players[game.currentTurn]?.address === userAddress;

  const handleDeal = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await api.requestDeal(tableId);

      // Fetch our private cards
      const cards = await api.getPlayerCards(tableId, "0");

      setGame((prev) => ({
        ...prev,
        phase: "preflop",
        boardCards: [],
        pot: 200, // Small + big blind
        handNumber: prev.handNumber + 1,
        lastTxHash: result.tx_hash ?? undefined,
        proofSize: result.proof_size,
        onChainConfirmed: !!result.tx_hash,
        players: prev.players.map((p) => {
          if (p.address === userAddress) {
            return { ...p, cards: [cards.card1, cards.card2] as [number, number], folded: false, betThisRound: 0 };
          }
          return { ...p, folded: false, betThisRound: 0, cards: undefined };
        }),
      }));
    } catch (e) {
      setError(e instanceof Error ? e.message : "Deal failed");
    } finally {
      setLoading(false);
    }
  }, [tableId, userAddress]);

  const handleAction = useCallback(
    async (action: string, amount?: number) => {
      if (action === "start") {
        // Initialize players for demo
        setGame((prev) => ({
          ...prev,
          players: [
            {
              address: "player_0",
              seat: 0,
              stack: 10000,
              betThisRound: 0,
              folded: false,
              allIn: false,
            },
            {
              address: "player_1",
              seat: 1,
              stack: 10000,
              betThisRound: 0,
              folded: false,
              allIn: false,
            },
          ],
        }));
        await handleDeal();
        return;
      }

      setGame((prev) => {
        const players = [...prev.players];
        const me = players.findIndex((p) => p.address === userAddress);
        if (me === -1) return prev;

        switch (action) {
          case "fold":
            players[me] = { ...players[me], folded: true };
            return { ...prev, players, phase: "settlement" };

          case "check":
            return {
              ...prev,
              players,
              currentTurn: (prev.currentTurn + 1) % players.length,
            };

          case "call":
          case "raise":
          case "allin": {
            const betAmount = amount || 0;
            players[me] = {
              ...players[me],
              stack: players[me].stack - betAmount,
              betThisRound: players[me].betThisRound + betAmount,
              allIn: action === "allin",
            };
            return {
              ...prev,
              players,
              pot: prev.pot + betAmount,
              currentTurn: (prev.currentTurn + 1) % players.length,
            };
          }

          default:
            return prev;
        }
      });
    },
    [userAddress, handleDeal]
  );

  // Reveal board cards when phase transitions
  useEffect(() => {
    if (game.phase === "preflop" && game.boardCards.length === 0) {
      // Auto-reveal flop after preflop betting (demo: after a short delay)
    }
  }, [game.phase, game.boardCards.length]);

  const handleReveal = useCallback(
    async (phase: "flop" | "turn" | "river") => {
      setLoading(true);
      try {
        const result = await api.requestReveal(tableId, phase);
        setGame((prev) => ({
          ...prev,
          phase,
          boardCards: [...prev.boardCards, ...result.cards],
          lastTxHash: result.tx_hash ?? prev.lastTxHash,
          proofSize: result.proof_size,
          onChainConfirmed: !!result.tx_hash || prev.onChainConfirmed,
        }));
      } catch (e) {
        setError(e instanceof Error ? e.message : "Reveal failed");
      } finally {
        setLoading(false);
      }
    },
    [tableId]
  );

  const handleShowdown = useCallback(async () => {
    setLoading(true);
    try {
      const result = await api.requestShowdown(tableId);
      setGame((prev) => ({
        ...prev,
        phase: "settlement",
        lastTxHash: result.tx_hash ?? prev.lastTxHash,
        proofSize: result.proof_size,
        onChainConfirmed: !!result.tx_hash || prev.onChainConfirmed,
      }));
    } catch (e) {
      setError(e instanceof Error ? e.message : "Showdown failed");
    } finally {
      setLoading(false);
    }
  }, [tableId]);

  const currentBet = Math.max(...game.players.map((p) => p.betThisRound), 0);

  return (
    <div className="flex flex-col items-center gap-6 min-h-screen bg-gray-900 p-4">
      {/* Header */}
      <div className="flex items-center justify-between w-full max-w-3xl">
        <h1 className="text-xl font-bold text-white">
          Stellar Poker - Table #{tableId}
        </h1>
        <div className="flex items-center gap-2">
          <div className="text-sm text-gray-400">
            Hand #{game.handNumber} | Phase: {game.phase}
          </div>
          {loading && (
            <div className="w-4 h-4 border-2 border-yellow-400 border-t-transparent rounded-full animate-spin" />
          )}
        </div>
      </div>

      {error && (
        <div className="bg-red-900/50 text-red-300 px-4 py-2 rounded-lg text-sm">
          {error}
        </div>
      )}

      {/* Table felt */}
      <div className="relative w-full max-w-3xl aspect-[16/10] bg-gradient-to-b from-green-900 to-green-800 rounded-[60px] border-8 border-brown-800 shadow-2xl flex flex-col items-center justify-center gap-4"
        style={{ borderColor: "#5D4037" }}
      >
        {/* Opponent seats (top) */}
        <div className="flex gap-8 -mt-16">
          {game.players
            .filter((p) => p.address !== userAddress)
            .map((player) => (
              <PlayerSeat
                key={player.address}
                player={player}
                isCurrentTurn={
                  game.players[game.currentTurn]?.address === player.address
                }
                isDealer={player.seat === game.dealerSeat}
                isUser={false}
              />
            ))}
        </div>

        {/* Board cards and pot */}
        <Board cards={game.boardCards} pot={game.pot} />

        {/* Dev controls for board reveal */}
        {game.phase === "preflop" && (
          <button
            onClick={() => handleReveal("flop")}
            className="text-xs bg-gray-700 hover:bg-gray-600 text-gray-300 px-3 py-1 rounded"
          >
            Deal Flop
          </button>
        )}
        {game.phase === "flop" && (
          <button
            onClick={() => handleReveal("turn")}
            className="text-xs bg-gray-700 hover:bg-gray-600 text-gray-300 px-3 py-1 rounded"
          >
            Deal Turn
          </button>
        )}
        {game.phase === "turn" && (
          <button
            onClick={() => handleReveal("river")}
            className="text-xs bg-gray-700 hover:bg-gray-600 text-gray-300 px-3 py-1 rounded"
          >
            Deal River
          </button>
        )}
        {game.phase === "river" && (
          <button
            onClick={handleShowdown}
            className="text-xs bg-purple-700 hover:bg-purple-600 text-white px-3 py-1 rounded font-medium"
          >
            Showdown
          </button>
        )}

        {/* User seat (bottom) */}
        <div className="flex gap-8 -mb-16">
          {userPlayer && (
            <PlayerSeat
              player={userPlayer}
              isCurrentTurn={isMyTurn}
              isDealer={userPlayer.seat === game.dealerSeat}
              isUser={true}
            />
          )}
        </div>
      </div>

      {/* Action panel */}
      <div className="w-full max-w-3xl">
        <ActionPanel
          phase={game.phase}
          isMyTurn={isMyTurn}
          currentBet={currentBet}
          myBet={userPlayer?.betThisRound || 0}
          myStack={userPlayer?.stack || 0}
          onAction={handleAction}
          onChainConfirmed={game.onChainConfirmed}
        />
      </div>

      {/* MPC Status footer */}
      <div className="text-xs text-gray-500 flex flex-col items-center gap-1">
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 rounded-full bg-green-500" />
          MPC Committee: 3/3 nodes online | TACEO coNoir REP3
          {game.proofSize && (
            <span className="bg-gray-700 text-gray-300 px-2 py-0.5 rounded text-[10px]">
              Proof: {(game.proofSize / 1024).toFixed(1)}KB
            </span>
          )}
        </div>
        {game.lastTxHash && (
          <div className="flex items-center gap-1">
            {game.onChainConfirmed ? (
              <span className="text-green-400">&#10003;</span>
            ) : (
              <span className="text-yellow-400">&#9679;</span>
            )}
            <span className="text-gray-400">
              Tx:{" "}
              <a
                href={`https://stellar.expert/explorer/testnet/tx/${game.lastTxHash}`}
                target="_blank"
                rel="noopener noreferrer"
                className="text-blue-400 hover:underline"
              >
                {game.lastTxHash.slice(0, 8)}...{game.lastTxHash.slice(-8)}
              </a>
            </span>
          </div>
        )}
      </div>
    </div>
  );
}
