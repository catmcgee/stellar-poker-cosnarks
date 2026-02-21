"use client";

import { useState, useEffect, useCallback } from "react";
import Link from "next/link";
import { Board } from "./Board";
import { Card } from "./Card";
import { PlayerSeat } from "./PlayerSeat";
import { ActionPanel } from "./ActionPanel";
import { PixelWorld } from "./PixelWorld";
import { PixelCat, PixelHeart } from "./PixelCat";
import type { GameState, GamePhase } from "@/lib/game-state";
import { createInitialState } from "@/lib/game-state";
import * as api from "@/lib/api";
import { connectFreighterWallet, type WalletSession } from "@/lib/freighter";

interface TableProps {
  tableId: number;
}

type ActiveRequest = "deal" | "flop" | "turn" | "river" | "showdown" | null;

function isStellarAddress(address: string): boolean {
  return /^G[A-Z2-7]{55}$/.test(address.trim());
}

function shortAddress(address: string): string {
  return `${address.slice(0, 6)}...${address.slice(-6)}`;
}

function normalizeTxHash(hash: string | null | undefined): string | undefined {
  if (!hash) return undefined;
  if (hash === "null" || hash === "submitted") return undefined;
  return hash;
}

function toNumber(value: unknown, fallback: number): number {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string") {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return fallback;
}

function mapOnChainPhase(phase: string): GamePhase | null {
  switch (phase) {
    case "Waiting":
      return "waiting";
    case "Dealing":
      return "dealing";
    case "Preflop":
      return "preflop";
    case "Flop":
      return "flop";
    case "Turn":
      return "turn";
    case "River":
      return "river";
    case "Showdown":
      return "showdown";
    case "Settlement":
      return "settlement";
    case "DealingFlop":
      return "preflop";
    case "DealingTurn":
      return "flop";
    case "DealingRiver":
      return "turn";
    default:
      return null;
  }
}

export function Table({ tableId }: TableProps) {
  const [game, setGame] = useState<GameState>(() => createInitialState(tableId));
  const [wallet, setWallet] = useState<WalletSession | null>(null);
  const [opponentAddress, setOpponentAddress] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [connectingWallet, setConnectingWallet] = useState(false);
  const [activeRequest, setActiveRequest] = useState<ActiveRequest>(null);
  const [onChainPhase, setOnChainPhase] = useState<string>("unknown");
  const [winnerAddress, setWinnerAddress] = useState<string | null>(null);

  const userAddress = wallet?.address;
  const userPlayer = userAddress
    ? game.players.find((p) => p.address === userAddress)
    : undefined;
  const isMyTurn = !!userAddress && game.players[game.currentTurn]?.address === userAddress;

  const syncOnChainState = useCallback(async () => {
    try {
      const { parsed } = await api.getParsedTableState(tableId);
      if (!parsed) return;

      const phaseRaw = typeof parsed.phase === "string" ? parsed.phase : null;
      if (phaseRaw) {
        setOnChainPhase(phaseRaw);
      }
      const mappedPhase = phaseRaw ? mapOnChainPhase(phaseRaw) : null;

      const boardCards = Array.isArray(parsed.board_cards)
        ? parsed.board_cards
            .map((v) => toNumber(v, -1))
            .filter((v) => v >= 0)
        : null;

      const rawPlayers = Array.isArray(parsed.players)
        ? (parsed.players as Array<Record<string, unknown>>)
        : null;

      setGame((prev) => {
        const mergedPlayers =
          rawPlayers && rawPlayers.length > 0
            ? rawPlayers.map((raw, index) => {
                const address =
                  typeof raw.address === "string"
                    ? raw.address
                    : prev.players[index]?.address ?? `seat-${index}`;
                const existing = prev.players.find((p) => p.address === address);
                return {
                  address,
                  seat: toNumber(raw.seat_index, existing?.seat ?? index),
                  stack: toNumber(raw.stack, existing?.stack ?? 0),
                  betThisRound: toNumber(raw.bet_this_round, existing?.betThisRound ?? 0),
                  folded: Boolean(raw.folded),
                  allIn: Boolean(raw.all_in),
                  cards: existing?.cards,
                };
              })
            : prev.players;

        return {
          ...prev,
          phase: mappedPhase ?? prev.phase,
          boardCards: boardCards ?? prev.boardCards,
          pot: toNumber(parsed.pot, prev.pot),
          currentTurn: toNumber(parsed.current_turn, prev.currentTurn),
          dealerSeat: toNumber(parsed.dealer_seat, prev.dealerSeat),
          handNumber: toNumber(parsed.hand_number, prev.handNumber),
          players: mergedPlayers,
        };
      });
    } catch {
      // Non-fatal; UI still works off latest known state.
    }
  }, [tableId]);

  const hydrateMyCards = useCallback(
    async (auth: WalletSession) => {
      try {
        const cards = await api.getPlayerCards(tableId, auth.address, auth);
        setGame((prev) => ({
          ...prev,
          players: prev.players.map((p) =>
            p.address === auth.address
              ? { ...p, cards: [cards.card1, cards.card2] }
              : p
          ),
        }));
      } catch {
        // Cards may not be available yet; keep UI usable.
      }
    },
    [tableId]
  );

  useEffect(() => {
    void syncOnChainState();
    const interval = setInterval(() => {
      void syncOnChainState();
    }, 4000);
    return () => clearInterval(interval);
  }, [syncOnChainState]);

  const handleConnectWallet = useCallback(async () => {
    setConnectingWallet(true);
    setError(null);
    try {
      const connected = await connectFreighterWallet();
      setWallet(connected);
      if (!opponentAddress && game.players.length >= 2) {
        const existingOpponent = game.players.find((p) => p.address !== connected.address);
        if (existingOpponent) {
          setOpponentAddress(existingOpponent.address);
        }
      }
      await hydrateMyCards(connected);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to connect wallet");
    } finally {
      setConnectingWallet(false);
    }
  }, [game.players, hydrateMyCards, opponentAddress]);

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
      setActiveRequest("deal");
      setError(null);
      setWinnerAddress(null);

      try {
        const result = await api.requestDeal(tableId, players, wallet);
        const txHash = normalizeTxHash(result.tx_hash);

        setGame((prev) => ({
          ...prev,
          phase: "preflop",
          boardCards: [],
          handNumber: prev.handNumber + 1,
          lastTxHash: txHash,
          proofSize: result.proof_size,
          onChainConfirmed: !!txHash,
          players: players.map((address, seat) => ({
            address,
            seat,
            stack: 10000,
            betThisRound: 0,
            folded: false,
            allIn: false,
          })),
        }));

        await hydrateMyCards(wallet);
        await syncOnChainState();
      } catch (e) {
        setError(e instanceof Error ? e.message : "Deal failed");
      } finally {
        setLoading(false);
        setActiveRequest(null);
      }
    },
    [hydrateMyCards, syncOnChainState, tableId, wallet]
  );

  const handleAction = useCallback(
    async (action: string) => {
      if (action !== "start") {
        setError("Use the DEAL/REVEAL/SHOWDOWN controls on the table.");
        return;
      }

      const players = resolvePlayersForDeal();
      if (!players) {
        return;
      }
      await handleDeal(players);
    },
    [resolvePlayersForDeal, handleDeal]
  );

  const handleReveal = useCallback(
    async (phase: "flop" | "turn" | "river") => {
      if (!wallet) {
        setError("Connect Freighter wallet before requesting reveal");
        return;
      }

      setLoading(true);
      setActiveRequest(phase);
      setError(null);
      try {
        const result = await api.requestReveal(tableId, phase, wallet);
        const txHash = normalizeTxHash(result.tx_hash);
        setGame((prev) => ({
          ...prev,
          phase,
          boardCards: [...prev.boardCards, ...result.cards],
          lastTxHash: txHash ?? prev.lastTxHash,
          proofSize: result.proof_size,
          onChainConfirmed: !!txHash || prev.onChainConfirmed,
        }));
        await syncOnChainState();
      } catch (e) {
        setError(e instanceof Error ? e.message : "Reveal failed");
      } finally {
        setLoading(false);
        setActiveRequest(null);
      }
    },
    [syncOnChainState, tableId, wallet]
  );

  const handleShowdown = useCallback(async () => {
    if (!wallet) {
      setError("Connect Freighter wallet before requesting showdown");
      return;
    }

    setLoading(true);
    setActiveRequest("showdown");
    setError(null);
    try {
      const result = await api.requestShowdown(tableId, wallet);
      const txHash = normalizeTxHash(result.tx_hash);
      setWinnerAddress(result.winner);
      setGame((prev) => ({
        ...prev,
        phase: "settlement",
        lastTxHash: txHash ?? prev.lastTxHash,
        proofSize: result.proof_size,
        onChainConfirmed: !!txHash || prev.onChainConfirmed,
      }));
      await syncOnChainState();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Showdown failed");
    } finally {
      setLoading(false);
      setActiveRequest(null);
    }
  }, [syncOnChainState, tableId, wallet]);

  const currentBet = Math.max(...game.players.map((p) => p.betThisRound), 0);

  const dealerLine = (() => {
    if (loading) {
      switch (activeRequest) {
        case "deal":
          return "Dealer: Shuffling, dealing, and proving the hand...";
        case "flop":
          return "Dealer: Burning one, turning over the flop...";
        case "turn":
          return "Dealer: Turn card coming up...";
        case "river":
          return "Dealer: Final card on the river...";
        case "showdown":
          return "Dealer: Reading hands and verifying showdown proof (slowest step).";
        default:
          return "Dealer: One moment...";
      }
    }

    switch (game.phase) {
      case "waiting":
        return "Dealer: Connect wallet, seat your opponent, then click DEAL CARDS.";
      case "dealing":
        return "Dealer: Cards are being dealt.";
      case "preflop":
        return "Dealer: Preflop is live. Click DEAL FLOP when ready.";
      case "flop":
        return "Dealer: Flop is out. Click DEAL TURN.";
      case "turn":
        return "Dealer: Turn card is out. Click DEAL RIVER.";
      case "river":
        return "Dealer: River is out. Click SHOWDOWN.";
      case "showdown":
        return "Dealer: Showdown in progress.";
      case "settlement":
        return winnerAddress
          ? `Dealer: Hand complete. Winner: ${shortAddress(winnerAddress)}.`
          : "Dealer: Hand complete. Start the next hand when ready.";
      default:
        return "Dealer: Ready when you are.";
    }
  })();

  return (
    <PixelWorld>
      <div className="min-h-screen flex flex-col items-center gap-4 p-4 pt-6 relative z-[10]">
        {/* Header bar */}
        <div className="w-full max-w-3xl flex items-center justify-between">
          <div className="flex items-center gap-3">
            <Link
              href="/"
              className="text-[14px]"
              style={{
                color: "#f5e6c8",
                textShadow: "2px 2px 0 #2c3e50",
                textDecoration: "none",
                fontFamily: "'Press Start 2P', monospace",
              }}
            >
              ←
            </Link>
            <PixelHeart size={3} beating />
            <h1
              className="text-[10px]"
              style={{
                color: "white",
                textShadow: "2px 2px 0 #2c3e50",
              }}
            >
              TABLE #{tableId}
            </h1>
          </div>

          <div className="flex items-center gap-3">
            <div className="text-[7px]" style={{ color: "#c8e6ff" }}>
              HAND #{game.handNumber} | {game.phase.toUpperCase()} | CHAIN {onChainPhase}
            </div>

            {wallet ? (
              <div
                className="pixel-border-thin px-2 py-1"
                style={{
                  background: "rgba(39, 174, 96, 0.2)",
                  fontSize: "7px",
                  color: "#27ae60",
                }}
              >
                {shortAddress(wallet.address)}
              </div>
            ) : (
              <button
                onClick={handleConnectWallet}
                disabled={connectingWallet}
                className="pixel-btn pixel-btn-blue text-[7px]"
                style={{ padding: "4px 10px" }}
              >
                {connectingWallet ? "..." : "CONNECT"}
              </button>
            )}

            {loading && (
              <div
                style={{
                  width: "12px",
                  height: "12px",
                  border: "2px solid #f1c40f",
                  borderTopColor: "transparent",
                  borderRadius: "50%",
                  animation: "spin 0.6s linear infinite",
                }}
              />
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
              style={{ padding: "6px 10px" }}
            />
          </div>
        )}

        {/* Dealer line */}
        <div
          className="w-full max-w-3xl pixel-border-thin px-4 py-2"
          style={{
            background: "rgba(20, 12, 8, 0.75)",
            borderColor: "#8b6914",
          }}
        >
          <span className="text-[7px]" style={{ color: "#f5e6c8" }}>
            {dealerLine}
          </span>
        </div>

        {/* Error display */}
        {error && (
          <div
            className="pixel-border-thin px-4 py-2"
            style={{
              background: "rgba(231, 76, 60, 0.2)",
              borderColor: "#e74c3c",
            }}
          >
            <span className="text-[7px]" style={{ color: "#e74c3c" }}>
              {error}
            </span>
          </div>
        )}

        {/* ═══ THE POKER TABLE ═══ */}
        <div className="w-full max-w-3xl relative" style={{ minHeight: "400px" }}>
          <div
            className="pixel-border relative w-full flex flex-col items-center justify-center gap-4"
            style={{
              background:
                "radial-gradient(ellipse at center, var(--felt-light) 0%, var(--felt-mid) 40%, var(--felt-dark) 100%)",
              borderColor: "#6b4f12",
              padding: "40px 20px 40px 20px",
              minHeight: "360px",
              boxShadow:
                "inset 0 0 60px rgba(0,0,0,0.3), 0 8px 0 0 rgba(0,0,0,0.4), inset -4px -4px 0px 0px rgba(0,0,0,0.3), inset 4px 4px 0px 0px rgba(255,255,255,0.1)",
            }}
          >
            <div
              className="absolute inset-2 pointer-events-none"
              style={{
                border: "2px solid rgba(139, 105, 20, 0.3)",
              }}
            />

            {/* ── OPPONENTS (top) ── */}
            <div className="flex gap-8 items-end">
              {game.players
                .filter((p) => !userAddress || p.address !== userAddress)
                .map((player) => (
                  <PlayerSeat
                    key={player.address}
                    player={player}
                    isCurrentTurn={game.players[game.currentTurn]?.address === player.address}
                    isDealer={player.seat === game.dealerSeat}
                    isUser={false}
                    isWinner={!!winnerAddress && player.address === winnerAddress}
                  />
                ))}

              {game.players.filter((p) => !userAddress || p.address !== userAddress).length === 0 && (
                <>
                  {[
                    { variant: "grey" as const, flipped: false },
                    { variant: "black" as const, flipped: true },
                  ].map((seat, i) => (
                    <div key={i} className="flex flex-col items-center gap-2" style={{ opacity: 0.25 }}>
                      <PixelCat variant={seat.variant} size={4} flipped={seat.flipped} />
                      <div className="flex gap-1">
                        <Card faceDown size="sm" />
                        <Card faceDown size="sm" />
                      </div>
                      <div className="text-[6px]" style={{ color: 'rgba(255,255,255,0.3)' }}>
                        EMPTY
                      </div>
                    </div>
                  ))}
                </>
              )}
            </div>

            {/* ── BOARD (center) ── */}
            <div className="w-full flex flex-col items-center gap-2 my-2" style={{
              borderTop: '2px solid rgba(139, 105, 20, 0.2)',
              borderBottom: '2px solid rgba(139, 105, 20, 0.2)',
              padding: '12px 0',
            }}>
              <Board cards={game.boardCards} pot={game.pot} />

              {/* Phase action buttons */}
              <div className="flex gap-2 mt-1">
                {game.phase === "preflop" && (
                  <button
                    onClick={() => handleReveal("flop")}
                    disabled={loading}
                    className="pixel-btn pixel-btn-dark text-[7px]"
                    style={{ padding: "4px 12px", opacity: loading ? 0.7 : 1 }}
                  >
                    DEAL FLOP
                  </button>
                )}
                {game.phase === "flop" && (
                  <button
                    onClick={() => handleReveal("turn")}
                    disabled={loading}
                    className="pixel-btn pixel-btn-dark text-[7px]"
                    style={{ padding: "4px 12px", opacity: loading ? 0.7 : 1 }}
                  >
                    DEAL TURN
                  </button>
                )}
                {game.phase === "turn" && (
                  <button
                    onClick={() => handleReveal("river")}
                    disabled={loading}
                    className="pixel-btn pixel-btn-dark text-[7px]"
                    style={{ padding: "4px 12px", opacity: loading ? 0.7 : 1 }}
                  >
                    DEAL RIVER
                  </button>
                )}
                {game.phase === "river" && (
                  <button
                    onClick={handleShowdown}
                    disabled={loading}
                    className="pixel-btn pixel-btn-gold text-[7px]"
                    style={{ padding: "4px 12px", opacity: loading ? 0.7 : 1 }}
                  >
                    SHOWDOWN
                  </button>
                )}
              </div>
            </div>

            {/* ── YOU (bottom) ── */}
            <div className="flex gap-4 items-start">
              {userPlayer ? (
                <PlayerSeat
                  player={userPlayer}
                  isCurrentTurn={isMyTurn}
                  isDealer={userPlayer.seat === game.dealerSeat}
                  isUser={true}
                  isWinner={!!winnerAddress && userPlayer.address === winnerAddress}
                />
              ) : (
                <div className="flex flex-col items-center gap-2" style={{ opacity: 0.25 }}>
                  <PixelCat variant="orange" size={6} />
                  <div className="flex gap-1">
                    <Card faceDown size="md" />
                    <Card faceDown size="md" />
                  </div>
                  <div className="text-[7px]" style={{ color: 'rgba(255,255,255,0.3)' }}>
                    {wallet ? "WAITING TO JOIN..." : "CONNECT WALLET"}
                  </div>
                </div>
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
            <div
              style={{
                width: "6px",
                height: "6px",
                background: "#27ae60",
                boxShadow: "0 0 4px #27ae60",
              }}
            />
            <span className="text-[6px]" style={{ color: "#7f8c8d" }}>
              MPC: 3/3 NODES | TACEO CO-NOIR REP3
            </span>
            {game.proofSize && (
              <span
                className="pixel-border-thin px-1 py-0.5 text-[6px]"
                style={{
                  background: "rgba(20, 12, 8, 0.6)",
                  color: "#95a5a6",
                }}
              >
                PROOF: {(game.proofSize / 1024).toFixed(1)}KB
              </span>
            )}
          </div>
          {game.lastTxHash && (
            <div className="flex items-center gap-1">
              {game.onChainConfirmed ? (
                <PixelHeart size={2} />
              ) : (
                <div style={{ width: "4px", height: "4px", background: "#f1c40f" }} />
              )}
              <span className="text-[6px]" style={{ color: "#7f8c8d" }}>
                TX:{" "}
                <a
                  href={`https://stellar.expert/explorer/testnet/tx/${game.lastTxHash}`}
                  target="_blank"
                  rel="noopener noreferrer"
                  style={{ color: "#3498db" }}
                >
                  {game.lastTxHash.slice(0, 8)}...{game.lastTxHash.slice(-8)}
                </a>
              </span>
            </div>
          )}
        </div>

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
