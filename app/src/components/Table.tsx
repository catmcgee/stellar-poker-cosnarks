"use client";

import { useState, useEffect, useCallback } from "react";
import { Board } from "./Board";
import { PlayerSeat } from "./PlayerSeat";
import { ActionPanel } from "./ActionPanel";
import { PixelWorld } from "./PixelWorld";
import { PixelCat, PixelHeart } from "./PixelCat";
import type { GameState } from "@/lib/game-state";
import { createInitialState } from "@/lib/game-state";
import * as api from "@/lib/api";
import { connectFreighterWallet, type WalletSession } from "@/lib/freighter";

interface TableProps {
  tableId: number;
}

function isStellarAddress(address: string): boolean {
  return /^G[A-Z2-7]{55}$/.test(address.trim());
}

function shortAddress(address: string): string {
  return `${address.slice(0, 6)}...${address.slice(-6)}`;
}

export function Table({ tableId }: TableProps) {
  const [game, setGame] = useState<GameState>(() => createInitialState(tableId));
  const [wallet, setWallet] = useState<WalletSession | null>(null);
  const [opponentAddress, setOpponentAddress] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [connectingWallet, setConnectingWallet] = useState(false);

  const userAddress = wallet?.address;
  const userPlayer = userAddress
    ? game.players.find((p) => p.address === userAddress)
    : undefined;
  const isMyTurn = !!userAddress && game.players[game.currentTurn]?.address === userAddress;

  const handleConnectWallet = useCallback(async () => {
    setConnectingWallet(true);
    setError(null);
    try {
      const connected = await connectFreighterWallet();
      setWallet(connected);
      if (!opponentAddress && game.players.length >= 2) {
        const existingOpponent = game.players.find(
          (p) => p.address !== connected.address
        );
        if (existingOpponent) {
          setOpponentAddress(existingOpponent.address);
        }
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to connect wallet");
    } finally {
      setConnectingWallet(false);
    }
  }, [game.players, opponentAddress]);

  const resolvePlayersForDeal = useCallback((): string[] | null => {
    if (!wallet) {
      setError("Connect Freighter wallet before starting a hand");
      return null;
    }

    const existingOpponent = game.players.find((p) => p.address !== wallet.address)?.address;
    const opponent = (existingOpponent ?? opponentAddress).trim();

    if (!isStellarAddress(wallet.address)) {
      setError("Connected wallet address is invalid");
      return null;
    }
    if (!isStellarAddress(opponent)) {
      setError("Enter a valid opponent Stellar address");
      return null;
    }
    if (opponent === wallet.address) {
      setError("Opponent address must be different from your wallet");
      return null;
    }

    return [wallet.address, opponent];
  }, [wallet, game.players, opponentAddress]);

  const handleDeal = useCallback(
    async (players: string[]) => {
      if (!wallet) {
        setError("Connect Freighter wallet before dealing");
        return;
      }

      setLoading(true);
      setError(null);
      try {
        const result = await api.requestDeal(tableId, players, wallet);

        setGame((prev) => ({
          ...prev,
          phase: "preflop",
          boardCards: [],
          pot: 200,
          handNumber: prev.handNumber + 1,
          lastTxHash: result.tx_hash ?? undefined,
          proofSize: result.proof_size,
          onChainConfirmed: !!result.tx_hash,
          players: players.map((address, seat) => ({
            address,
            seat,
            stack: 10000,
            betThisRound: 0,
            folded: false,
            allIn: false,
          })),
        }));
      } catch (e) {
        setError(e instanceof Error ? e.message : "Deal failed");
      } finally {
        setLoading(false);
      }
    },
    [tableId, wallet]
  );

  const handleAction = useCallback(
    async (action: string, amount?: number) => {
      if (action === "start") {
        const players = resolvePlayersForDeal();
        if (!players) {
          return;
        }

        setGame((prev) => ({
          ...prev,
          players: players.map((address, seat) => ({
            address,
            seat,
            stack: 10000,
            betThisRound: 0,
            folded: false,
            allIn: false,
          })),
        }));

        await handleDeal(players);
        return;
      }

      if (!userAddress) {
        setError("Connect Freighter wallet to act");
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
    [resolvePlayersForDeal, handleDeal, userAddress]
  );

  useEffect(() => {
    if (game.phase === "preflop" && game.boardCards.length === 0) {
      // Auto-reveal could be driven here
    }
  }, [game.phase, game.boardCards.length]);

  const handleReveal = useCallback(
    async (phase: "flop" | "turn" | "river") => {
      if (!wallet) {
        setError("Connect Freighter wallet before requesting reveal");
        return;
      }

      setLoading(true);
      try {
        const result = await api.requestReveal(tableId, phase, wallet);
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
    [tableId, wallet]
  );

  const handleShowdown = useCallback(async () => {
    if (!wallet) {
      setError("Connect Freighter wallet before requesting showdown");
      return;
    }

    setLoading(true);
    try {
      const result = await api.requestShowdown(tableId, wallet);
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
  }, [tableId, wallet]);

  const currentBet = Math.max(...game.players.map((p) => p.betThisRound), 0);

  return (
    <PixelWorld>
      <div className="min-h-screen flex flex-col items-center gap-4 p-4 pt-6 relative z-[10]">

        {/* Header bar */}
        <div className="w-full max-w-3xl flex items-center justify-between">
          <div className="flex items-center gap-2">
            <PixelHeart size={3} beating />
            <h1 className="text-[10px]" style={{
              color: 'white',
              textShadow: '2px 2px 0 #2c3e50',
            }}>
              TABLE #{tableId}
            </h1>
          </div>

          <div className="flex items-center gap-3">
            <div className="text-[7px]" style={{ color: '#c8e6ff' }}>
              HAND #{game.handNumber} | {game.phase.toUpperCase()}
            </div>

            {wallet ? (
              <div className="pixel-border-thin px-2 py-1" style={{
                background: 'rgba(39, 174, 96, 0.2)',
                fontSize: '7px',
                color: '#27ae60',
              }}>
                {shortAddress(wallet.address)}
              </div>
            ) : (
              <button
                onClick={handleConnectWallet}
                disabled={connectingWallet}
                className="pixel-btn pixel-btn-blue text-[7px]"
                style={{ padding: '4px 10px' }}
              >
                {connectingWallet ? "..." : "CONNECT"}
              </button>
            )}

            {loading && (
              <div style={{
                width: '12px',
                height: '12px',
                border: '2px solid #f1c40f',
                borderTopColor: 'transparent',
                borderRadius: '50%',
                animation: 'spin 0.6s linear infinite',
              }} />
            )}
          </div>
        </div>

        {/* Opponent address input */}
        {game.phase === "waiting" && (
          <div className="w-full max-w-3xl flex items-center gap-2">
            <input
              type="text"
              value={opponentAddress}
              onChange={(e) => setOpponentAddress(e.target.value.trim())}
              placeholder="OPPONENT ADDRESS (G...)"
              className="flex-1 text-[7px]"
              style={{ padding: '6px 10px' }}
            />
          </div>
        )}

        {/* Error display */}
        {error && (
          <div className="pixel-border-thin px-4 py-2" style={{
            background: 'rgba(231, 76, 60, 0.2)',
            borderColor: '#e74c3c',
          }}>
            <span className="text-[7px]" style={{ color: '#e74c3c' }}>{error}</span>
          </div>
        )}

        {/* ═══ THE POKER TABLE ═══ */}
        <div className="w-full max-w-3xl relative" style={{ minHeight: '400px' }}>

          {/* Felt surface */}
          <div
            className="pixel-border relative w-full flex flex-col items-center justify-center gap-4"
            style={{
              background: `
                radial-gradient(ellipse at center, var(--felt-light) 0%, var(--felt-mid) 40%, var(--felt-dark) 100%)
              `,
              borderColor: '#6b4f12',
              padding: '40px 20px 40px 20px',
              minHeight: '360px',
              boxShadow: `
                inset 0 0 60px rgba(0,0,0,0.3),
                0 8px 0 0 rgba(0,0,0,0.4),
                inset -4px -4px 0px 0px rgba(0,0,0,0.3),
                inset 4px 4px 0px 0px rgba(255,255,255,0.1)
              `,
            }}
          >
            {/* Table edge decoration */}
            <div className="absolute inset-2 pointer-events-none" style={{
              border: '2px solid rgba(139, 105, 20, 0.3)',
            }} />

            {/* Opponent seats (top) */}
            <div className="flex gap-4 -mt-2">
              {game.players
                .filter((p) => !userAddress || p.address !== userAddress)
                .map((player) => (
                  <PlayerSeat
                    key={player.address}
                    player={player}
                    isCurrentTurn={game.players[game.currentTurn]?.address === player.address}
                    isDealer={player.seat === game.dealerSeat}
                    isUser={false}
                  />
                ))}
            </div>

            {/* Board */}
            <Board cards={game.boardCards} pot={game.pot} />

            {/* Phase action buttons */}
            <div className="flex gap-2">
              {game.phase === "preflop" && (
                <button
                  onClick={() => handleReveal("flop")}
                  className="pixel-btn pixel-btn-dark text-[7px]"
                  style={{ padding: '4px 12px' }}
                >
                  DEAL FLOP
                </button>
              )}
              {game.phase === "flop" && (
                <button
                  onClick={() => handleReveal("turn")}
                  className="pixel-btn pixel-btn-dark text-[7px]"
                  style={{ padding: '4px 12px' }}
                >
                  DEAL TURN
                </button>
              )}
              {game.phase === "turn" && (
                <button
                  onClick={() => handleReveal("river")}
                  className="pixel-btn pixel-btn-dark text-[7px]"
                  style={{ padding: '4px 12px' }}
                >
                  DEAL RIVER
                </button>
              )}
              {game.phase === "river" && (
                <button
                  onClick={handleShowdown}
                  className="pixel-btn pixel-btn-gold text-[7px]"
                  style={{ padding: '4px 12px' }}
                >
                  SHOWDOWN
                </button>
              )}
            </div>

            {/* User seat (bottom) */}
            <div className="flex gap-4 -mb-2">
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
        <div className="flex flex-col items-center gap-1 mt-2">
          <div className="flex items-center gap-2">
            <div style={{
              width: '6px',
              height: '6px',
              background: '#27ae60',
              boxShadow: '0 0 4px #27ae60',
            }} />
            <span className="text-[6px]" style={{ color: '#7f8c8d' }}>
              MPC: 3/3 NODES | TACEO CO-NOIR REP3
            </span>
            {game.proofSize && (
              <span className="pixel-border-thin px-1 py-0.5 text-[6px]" style={{
                background: 'rgba(20, 12, 8, 0.6)',
                color: '#95a5a6',
              }}>
                PROOF: {(game.proofSize / 1024).toFixed(1)}KB
              </span>
            )}
          </div>
          {game.lastTxHash && (
            <div className="flex items-center gap-1">
              {game.onChainConfirmed ? (
                <PixelHeart size={2} />
              ) : (
                <div style={{ width: '4px', height: '4px', background: '#f1c40f' }} />
              )}
              <span className="text-[6px]" style={{ color: '#7f8c8d' }}>
                TX:{" "}
                <a
                  href={`https://stellar.expert/explorer/testnet/tx/${game.lastTxHash}`}
                  target="_blank"
                  rel="noopener noreferrer"
                  style={{ color: '#3498db' }}
                >
                  {game.lastTxHash.slice(0, 8)}...{game.lastTxHash.slice(-8)}
                </a>
              </span>
            </div>
          )}
        </div>

        {/* Decorative cats at bottom of scene */}
        <div className="fixed bottom-[14%] left-[5%] z-[5]">
          <PixelCat variant="grey" size={4} />
        </div>
        <div className="fixed bottom-[13%] right-[5%] z-[5]">
          <PixelCat variant="black" size={5} flipped />
        </div>
      </div>
    </PixelWorld>
  );
}
