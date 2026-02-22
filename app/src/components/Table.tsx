"use client";

import { useState, useEffect, useCallback, useRef } from "react";
import Link from "next/link";
import { Board } from "./Board";
import { Card } from "./Card";
import { PlayerSeat } from "./PlayerSeat";
import { ActionPanel } from "./ActionPanel";
import { PixelWorld } from "./PixelWorld";
import { PixelCat } from "./PixelCat";
import { PixelChip } from "./PixelChip";
import type { GameState, GamePhase } from "@/lib/game-state";
import { createInitialState } from "@/lib/game-state";
import * as api from "@/lib/api";
import {
  trySilentReconnect,
  type WalletSession,
} from "@/lib/freighter";
import { GameBoyButton, GameBoyModal } from "./GameBoyModal";
import { usePokerActions } from "@/lib/use-poker-actions";
import { getDealerLine } from "@/lib/dealer-lines";

type ActiveRequest = "deal" | "flop" | "turn" | "river" | "showdown" | null;
type PlayMode = "single" | "headsup" | "multi";

interface TableProps {
  tableId: number;
  initialPlayMode?: PlayMode;
}

function isStellarAddress(address: string): boolean {
  return /^G[A-Z2-7]{55}$/.test(address.trim());
}

function shortAddress(address: string): string {
  return `${address.slice(0, 6)}...${address.slice(-6)}`;
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

export function Table({ tableId, initialPlayMode }: TableProps) {
  const [game, setGame] = useState<GameState>(() => createInitialState(tableId));
  const [wallet, setWallet] = useState<WalletSession | null>(null);
  const [playMode, setPlayMode] = useState<PlayMode>(initialPlayMode ?? "headsup");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [joiningTable, setJoiningTable] = useState(false);
  const [activeRequest, setActiveRequest] = useState<ActiveRequest>(null);
  const [onChainPhase, setOnChainPhase] = useState<string>("unknown");
  const [winnerAddress, setWinnerAddress] = useState<string | null>(null);
  const [lobby, setLobby] = useState<api.TableLobbyResponse | null>(null);
  const [botLine, setBotLine] = useState<string | null>(null);
  const [gameboyOpen, setGameboyOpen] = useState(false);
  const [elapsed, setElapsed] = useState(0);
  const autoStreetRef = useRef<string>("");
  const inferredModeRef = useRef(false);

  const userAddress = wallet?.address;
  const userPlayer = userAddress
    ? game.players.find((p) => p.address === userAddress)
    : undefined;
  const isSoloBettingPhase =
    playMode === "single" &&
    ["preflop", "flop", "turn", "river"].includes(game.phase);
  const onChainTurnAddress = game.players[game.currentTurn]?.address;
  const displayedTurnAddress = isSoloBettingPhase
    ? userAddress
    : onChainTurnAddress;
  const isMyTurn = !!userAddress && displayedTurnAddress === userAddress;
  const isWalletSeated = !!wallet && !!userPlayer;
  const seatedAddresses = game.players
    .filter((p) => isStellarAddress(p.address))
    .map((p) => p.address);
  const tableSeatLabel =
    seatedAddresses.length > 0
      ? seatedAddresses.map(shortAddress).join(" vs ")
      : "NO SEATS YET";

  const syncOnChainState = useCallback(async () => {
    try {
      const [tableState, lobbyState] = await Promise.all([
        api.getParsedTableState(tableId),
        api.getTableLobby(tableId).catch(() => null),
      ]);
      const { parsed } = tableState;
      if (!parsed) return;
      if (lobbyState) {
        setLobby(lobbyState);
      }

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
      const walletByChain = new Map<string, string>();
      if (lobbyState?.seats) {
        for (const seat of lobbyState.seats) {
          if (seat.wallet_address) {
            walletByChain.set(seat.chain_address, seat.wallet_address);
          }
        }
      }

      setGame((prev) => {
        const rawHasWallet =
          !!userAddress &&
          !!rawPlayers?.some((raw) => typeof raw.address === "string" && raw.address === userAddress);
        const prevHasWallet = !!userAddress && prev.players.some((p) => p.address === userAddress);
        const aliasWalletSeatForLocalDev =
          !!userAddress && !!rawPlayers && rawPlayers.length > 0 && !rawHasWallet && phaseRaw !== "Waiting";
        const preserveLocalSeatAddresses =
          !!userAddress &&
          prevHasWallet &&
          !!rawPlayers &&
          rawPlayers.length === prev.players.length &&
          !rawHasWallet &&
          prev.phase !== "waiting";

        const mergedPlayers =
          rawPlayers && rawPlayers.length > 0
            ? rawPlayers.map((raw, index) => {
                const chainAddress =
                  typeof raw.address === "string"
                    ? raw.address
                    : prev.players[index]?.address ?? `seat-${index}`;
                const lobbyAddress = walletByChain.get(chainAddress);
                const address = preserveLocalSeatAddresses
                  ? prev.players[index]?.address ?? chainAddress
                  : lobbyAddress ?? chainAddress;
                const normalizedAddress =
                  aliasWalletSeatForLocalDev && index === 0 ? userAddress ?? address : address;
                const existing =
                  prev.players.find((p) => p.address === normalizedAddress) ?? prev.players[index];
                return {
                  address: normalizedAddress,
                  seat: toNumber(raw.seat_index, existing?.seat ?? index),
                  stack:
                    playMode === "single"
                      ? existing?.stack ?? 100
                      : toNumber(raw.stack, existing?.stack ?? 0),
                  betThisRound:
                    playMode === "single"
                      ? existing?.betThisRound ?? 0
                      : toNumber(raw.bet_this_round, existing?.betThisRound ?? 0),
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
          pot: playMode === "single" ? prev.pot : toNumber(parsed.pot, prev.pot),
          currentTurn: toNumber(parsed.current_turn, prev.currentTurn),
          dealerSeat: toNumber(parsed.dealer_seat, prev.dealerSeat),
          handNumber: toNumber(parsed.hand_number, prev.handNumber),
          players: mergedPlayers,
        };
      });
    } catch {
      // Non-fatal; UI still works off latest known state.
    }
  }, [playMode, tableId, userAddress]);

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

  const {
    handleJoinTable,
    handleReveal,
    handleShowdown,
    handleAction,
  } = usePokerActions({
    tableId,
    wallet,
    playMode,
    game,
    lobby,
    setGame,
    setError,
    setLoading,
    setActiveRequest,
    setWinnerAddress,
    setBotLine,
    setJoiningTable,
    syncOnChainState,
    hydrateMyCards,
  });

  useEffect(() => {
    void syncOnChainState();
    const interval = setInterval(() => {
      void syncOnChainState();
    }, 4000);
    return () => clearInterval(interval);
  }, [syncOnChainState]);

  // Infer sensible default play mode from table capacity when no explicit mode was provided.
  useEffect(() => {
    if (inferredModeRef.current) return;
    if (initialPlayMode) {
      inferredModeRef.current = true;
      return;
    }
    if (!lobby) return;
    setPlayMode(lobby.max_players >= 3 ? "multi" : "headsup");
    inferredModeRef.current = true;
  }, [initialPlayMode, lobby]);

  // Silent reconnect on mount
  useEffect(() => {
    if (!wallet) {
      void trySilentReconnect().then((session) => {
        if (session) {
          setWallet(session);
        }
      });
    }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Elapsed timer while loading
  useEffect(() => {
    if (loading) {
      setElapsed(0);
      const interval = setInterval(() => {
        setElapsed((prev) => prev + 1);
      }, 1000);
      return () => clearInterval(interval);
    } else {
      setElapsed(0);
    }
  }, [loading]);

  const currentBet = Math.max(...game.players.map((p) => p.betThisRound), 0);
  const displayCurrentBet = currentBet;
  const displayPot = game.pot;
  const displayMyBet = userPlayer?.betThisRound || 0;
  const displayMyStack = userPlayer?.stack || 0;
  const canStartHand = !!wallet && isWalletSeated;
  const seatStatusHint =
    wallet && !isWalletSeated && seatedAddresses.length > 0
      ? "Connected wallet is not seated in this hand. Click JOIN TABLE first, then DEAL."
      : null;

  useEffect(() => {
    if (!wallet || loading) {
      return;
    }

    const key = `${game.handNumber}:${onChainPhase}`;
    let next: (() => Promise<void>) | null = null;

    switch (onChainPhase) {
      case "DealingFlop":
        next = async () => handleReveal("flop");
        break;
      case "DealingTurn":
        next = async () => handleReveal("turn");
        break;
      case "DealingRiver":
        next = async () => handleReveal("river");
        break;
      case "Showdown":
        next = handleShowdown;
        break;
      case "Waiting":
      case "Settlement":
      case "Preflop":
      case "Flop":
      case "Turn":
      case "River":
      case "Dealing":
        autoStreetRef.current = "";
        return;
      default:
        return;
    }

    if (!next || autoStreetRef.current === key) {
      return;
    }
    autoStreetRef.current = key;
    void next();
  }, [game.handNumber, handleReveal, handleShowdown, loading, onChainPhase, wallet]);

  const dealerLine = getDealerLine({
    loading,
    elapsed,
    activeRequest,
    playMode,
    botLine,
    onChainPhase,
    gamePhase: game.phase,
    wallet: !!wallet,
    isWalletSeated,
    seatedAddresses,
    tableSeatLabel,
    winnerAddress,
    userAddress,
    lobby,
  });

  return (
    <PixelWorld>
      <div className="min-h-screen flex flex-col items-center gap-4 p-4 pt-6 relative z-[10]">
        {/* Header bar */}
        <div className="w-full max-w-3xl flex items-center justify-between">
          <div className="flex items-center gap-3">
            <Link
              href="/"
              className="text-[24px]"
              style={{
                color: "#f5e6c8",
                textShadow: "2px 2px 0 #2c3e50",
                textDecoration: "none",
                fontFamily: "'Press Start 2P', monospace",
              }}
            >
              ←
            </Link>
            <h1
              className="text-[13px]"
              style={{
                color: "white",
                textShadow: "2px 2px 0 #2c3e50",
              }}
            >
              TABLE #{tableId}
            </h1>
            <GameBoyButton onClick={() => setGameboyOpen(true)} />
          </div>

          <div className="flex items-center gap-3">
            <div className="text-[9px]" style={{ color: "#c8e6ff" }}>
              HAND #{game.handNumber} | {game.phase.toUpperCase()}
            </div>

            {(() => {
              const explorerUrl = game.lastTxHash
                ? `https://stellar.expert/explorer/testnet/tx/${game.lastTxHash}`
                : wallet
                  ? `https://stellar.expert/explorer/testnet/account/${wallet.address}`
                  : null;
              if (!explorerUrl) return null;
              return (
                <a
                  href={explorerUrl}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-[9px]"
                  style={{
                    color: "#ffc078",
                    textDecoration: "none",
                    textShadow: "1px 1px 0 rgba(0,0,0,0.5)",
                  }}
                >
                  {game.lastTxHash ? "VIEW TX ↗" : "EXPLORER ↗"}
                </a>
              );
            })()}

            {wallet && (
              <div
                className="pixel-border-thin px-2 py-1"
                style={{
                  background: "rgba(39, 174, 96, 0.2)",
                  fontSize: "9px",
                  color: "#27ae60",
                }}
              >
                {shortAddress(wallet.address)}
              </div>
            )}
          </div>
        </div>


        {/* Dealer line */}
        <div
          className="w-full max-w-3xl pixel-border-thin px-4 py-2"
          style={{
            background: loading
              ? "rgba(40, 20, 8, 0.9)"
              : "rgba(12, 10, 24, 0.88)",
            borderColor: loading ? "#f1c40f" : "#c47d2e",
            animation: loading
              ? "dealerPulse 1.5s ease-in-out infinite"
              : undefined,
          }}
        >
          {loading && (
            <div className="flex items-center gap-2 mb-1">
              <div
                style={{
                  width: "8px",
                  height: "8px",
                  border: "2px solid #f1c40f",
                  borderTopColor: "transparent",
                  borderRadius: "50%",
                  animation: "spin 0.6s linear infinite",
                }}
              />
              <span
                className="text-[10px]"
                style={{ color: "#f1c40f", fontWeight: "bold" }}
              >
                GENERATING PROOF...
              </span>
            </div>
          )}
          <span
            className={loading ? "text-[10px]" : "text-[9px]"}
            style={{ color: loading ? "#ffeaa7" : "#f5e6c8" }}
          >
            {dealerLine}
          </span>
        </div>

        <style jsx>{`
          @keyframes dealerPulse {
            0%, 100% { opacity: 1; }
            50% { opacity: 0.85; }
          }
        `}</style>

        {/* Error display */}
        {error && (
          <div
            className="pixel-border-thin px-4 py-2"
            style={{
              background: "rgba(231, 76, 60, 0.2)",
              borderColor: "#e74c3c",
            }}
          >
            <span className="text-[9px]" style={{ color: "#e74c3c" }}>
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
            <div className="flex flex-wrap gap-6 items-end justify-center">
              {game.players
                .filter((p) => !userAddress || p.address !== userAddress)
                .map((player) => (
                  <PlayerSeat
                    key={player.address}
                    player={player}
                    isCurrentTurn={displayedTurnAddress === player.address}
                    isDealer={player.seat === game.dealerSeat}
                    isUser={false}
                    isWinner={!!winnerAddress && player.address === winnerAddress}
                    isBot={playMode === "single"}
                    hideChipStats={false}
                  />
                ))}

              {game.players.filter((p) => !userAddress || p.address !== userAddress).length === 0 && (
                <>
                  {[
                    { sprite: 17, flipped: false },
                    { sprite: 20, flipped: true },
                  ].map((seat, i) => (
                    <div key={i} className="flex flex-col items-center gap-2" style={{ opacity: 0.25 }}>
                      <PixelCat sprite={seat.sprite} size={48} flipped={seat.flipped} />
                      <div className="flex gap-1">
                        <Card faceDown size="sm" />
                        <Card faceDown size="sm" />
                      </div>
                      <div className="text-[8px]" style={{ color: 'rgba(255,255,255,0.3)' }}>
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
              <Board cards={game.boardCards} pot={displayPot} />

              {game.phase === "waiting" && wallet && !isWalletSeated && playMode !== "single" && (
                <button
                  onClick={() => void handleJoinTable()}
                  disabled={loading || joiningTable}
                  className="pixel-btn pixel-btn-blue text-[9px]"
                  style={{ padding: "6px 14px", opacity: loading || joiningTable ? 0.7 : 1 }}
                >
                  {joiningTable ? "JOINING..." : "JOIN TABLE"}
                </button>
              )}

              <div className="w-full max-w-xl mt-1">
                <ActionPanel
                  phase={game.phase}
                  isMyTurn={isMyTurn}
                  currentBet={displayCurrentBet}
                  myBet={displayMyBet}
                  myStack={displayMyStack}
                  onAction={handleAction}
                  onChainConfirmed={game.onChainConfirmed}
                  canStartHand={canStartHand}
                  canResolveShowdown={!!wallet}
                  statusHint={seatStatusHint}
                  loading={loading}
                  isSolo={playMode === "single"}
                />
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
                  hideChipStats={false}
                />
              ) : (
                <div className="flex flex-col items-center gap-2" style={{ opacity: 0.25 }}>
                  <PixelCat sprite={18} size={72} />
                  <div className="flex gap-1">
                    <Card faceDown size="md" />
                    <Card faceDown size="md" />
                  </div>
                  <div className="text-[9px]" style={{ color: 'rgba(255,255,255,0.3)' }}>
                    {wallet ? "WAITING TO JOIN..." : "CONNECT WALLET"}
                  </div>
                </div>
              )}
            </div>
          </div>
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
            <span className="text-[8px]" style={{ color: "#7f8c8d" }}>
              MPC: 3/3 NODES | TACEO CO-NOIR REP3
            </span>
          </div>
          {game.lastTxHash && (
            <div className="flex items-center gap-1">
              {game.onChainConfirmed ? (
                <PixelChip color="gold" size={2} />
              ) : (
                <div style={{ width: "4px", height: "4px", background: "#f1c40f" }} />
              )}
              <span className="text-[8px]" style={{ color: "#7f8c8d" }}>
                TX:{" "}
                <a
                  href={`https://stellar.expert/explorer/testnet/tx/${game.lastTxHash}`}
                  target="_blank"
                  rel="noopener noreferrer"
                  style={{ color: "#ffc078", textShadow: "1px 1px 0 rgba(0,0,0,0.5)" }}
                >
                  {game.lastTxHash.slice(0, 8)}...{game.lastTxHash.slice(-8)}
                </a>
              </span>
            </div>
          )}
        </div>

        <div className="fixed bottom-0 left-[5%] z-[5]" style={{ transform: 'translateY(15%)' }}>
          <PixelCat sprite={17} size={36} />
        </div>
        <div className="fixed bottom-0 right-[5%] z-[5]" style={{ transform: 'translateY(10%)' }}>
          <PixelCat sprite={21} size={48} flipped />
        </div>
      </div>

      <GameBoyModal
        open={gameboyOpen}
        onClose={() => setGameboyOpen(false)}
        onLogout={() => setWallet(null)}
      />
    </PixelWorld>
  );
}
