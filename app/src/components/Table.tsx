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

export function Table({ tableId, initialPlayMode }: TableProps) {
  const [game, setGame] = useState<GameState>(() => createInitialState(tableId));
  const [wallet, setWallet] = useState<WalletSession | null>(null);
  const [playMode, setPlayMode] = useState<PlayMode>(initialPlayMode ?? "single");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [joiningTable, setJoiningTable] = useState(false);
  const [activeRequest, setActiveRequest] = useState<ActiveRequest>(null);
  const [, setOnChainPhase] = useState<string>("unknown");
  const [winnerAddress, setWinnerAddress] = useState<string | null>(null);
  const [lobby, setLobby] = useState<api.TableLobbyResponse | null>(null);
  const [botLine, setBotLine] = useState<string | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const botTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const botStepRef = useRef<string>("");
  const botRetriesRef = useRef<Record<string, number>>({});

  const userAddress = wallet?.address;
  const userPlayer = userAddress
    ? game.players.find((p) => p.address === userAddress)
    : undefined;
  const isMyTurn = !!userAddress && game.players[game.currentTurn]?.address === userAddress;
  const isWalletSeated = !!wallet && !!userPlayer;
  const seatedAddresses = game.players
    .filter((p) => isStellarAddress(p.address))
    .map((p) => p.address);
  const tableSeatLabel =
    seatedAddresses.length > 0
      ? seatedAddresses.map(shortAddress).join(" vs ")
      : "NO SEATS YET";
  const claimedWallets = (lobby?.seats ?? [])
    .map((seat) => seat.wallet_address)
    .filter((address): address is string => !!address);

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
  }, [tableId, userAddress]);

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

  // Silent reconnect on mount
  useEffect(() => {
    if (!wallet) {
      void trySilentReconnect().then((session) => {
        if (session) {
          setWallet(session);
          void hydrateMyCards(session);
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

  const handleJoinTable = useCallback(async () => {
    if (!wallet) {
      setError("Connect Freighter wallet before joining a table");
      return;
    }
    setJoiningTable(true);
    setError(null);
    try {
      await api.joinTable(tableId, wallet);
      await syncOnChainState();
      await hydrateMyCards(wallet);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Join table failed");
    } finally {
      setJoiningTable(false);
    }
  }, [hydrateMyCards, syncOnChainState, tableId, wallet]);

  // Auto-join table when wallet is available but not yet seated
  const autoJoinedRef = useRef(false);
  useEffect(() => {
    if (
      wallet &&
      !isWalletSeated &&
      !joiningTable &&
      !autoJoinedRef.current &&
      game.phase === "waiting"
    ) {
      autoJoinedRef.current = true;
      void handleJoinTable();
    }
  }, [wallet, isWalletSeated, joiningTable, game.phase, handleJoinTable]);

  const resolvePlayersForDeal = useCallback((): string[] | null => {
    if (!wallet) {
      setError("Connect Freighter wallet before starting a hand");
      return null;
    }

    if (!isStellarAddress(wallet.address)) {
      setError("Connected wallet address is invalid");
      return null;
    }

    if (!claimedWallets.includes(wallet.address)) {
      setError("Join table first so your wallet is seated");
      return null;
    }

    const joinedWallets = lobby?.joined_wallets ?? claimedWallets.length;
    if (playMode === "headsup" && joinedWallets < 2) {
      setError("Two-player mode needs 2 joined wallets");
      return null;
    }
    if (playMode === "multi" && joinedWallets < 3) {
      setError("3-6 player mode needs at least 3 joined wallets");
      return null;
    }

    // Empty list tells coordinator to resolve all on-chain seats from lobby.
    return [];
  }, [wallet, claimedWallets, lobby, playMode]);

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
          players:
            players.length > 0
              ? players.map((address, seat) => ({
                  address,
                  seat,
                  stack: 10000,
                  betThisRound: 0,
                  folded: false,
                  allIn: false,
                }))
              : prev.players,
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
        if (playMode === "single") {
          botStepRef.current = "";
        }
      } finally {
        setLoading(false);
        setActiveRequest(null);
      }
    },
    [playMode, syncOnChainState, tableId, wallet]
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
      if (playMode === "single") {
        botStepRef.current = "";
      }
    } finally {
      setLoading(false);
      setActiveRequest(null);
    }
  }, [playMode, syncOnChainState, tableId, wallet]);

  const handleAction = useCallback(
    async (action: string) => {
      if (action === "showdown") {
        if (!wallet) {
          setError("Connect Freighter wallet before requesting showdown");
          return;
        }
        await handleShowdown();
        return;
      }

      const bettingActions = ["fold", "check", "call", "bet", "raise", "allin"];
      if (bettingActions.includes(action)) {
        if (playMode === "single") {
          console.log(`[single-player] betting action: ${action} (auto-handled by coordinator)`);
          return;
        }
        setError("Multiplayer betting actions are not yet wired to the API.");
        return;
      }

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
    [wallet, resolvePlayersForDeal, handleDeal, handleShowdown]
  );

  const currentBet = Math.max(...game.players.map((p) => p.betThisRound), 0);
  const canStartHand = !!wallet;
  const seatStatusHint =
    wallet && !isWalletSeated && seatedAddresses.length > 0
      ? "Connected wallet is not seated in this hand. Click JOIN TABLE first, then DEAL."
      : null;

  useEffect(() => {
    return () => {
      if (botTimerRef.current) {
        clearTimeout(botTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (playMode !== "single" || !wallet || loading) {
      return;
    }

    const stepKey = `${game.handNumber}:${game.phase}`;
    let line: string | null = null;
    let action: (() => Promise<void>) | null = null;

    switch (game.phase) {
      case "preflop":
        line = "Bot checks. Dealing flop...";
        action = async () => handleReveal("flop");
        break;
      case "flop":
        line = "Bot checks again. Dealing turn...";
        action = async () => handleReveal("turn");
        break;
      case "turn":
        line = "Bot checks again. Dealing river...";
        action = async () => handleReveal("river");
        break;
      case "river":
        line = "Bot always calls/checks. Going to showdown...";
        action = handleShowdown;
        break;
      case "showdown":
        line = "Bot tabled hand. Verifying showdown...";
        action = null;
        break;
      case "settlement":
      case "waiting":
        botStepRef.current = "";
        botRetriesRef.current = {};
        setBotLine(null);
        return;
      default:
        return;
    }

    setBotLine(line);
    if (!action) {
      return;
    }

    if (botStepRef.current === stepKey) {
      return;
    }

    const tries = botRetriesRef.current[stepKey] ?? 0;
    if (tries >= 2) {
      setBotLine("Bot paused after retries. Use the phase button once to continue.");
      return;
    }

    botStepRef.current = stepKey;
    botRetriesRef.current[stepKey] = tries + 1;

    if (botTimerRef.current) {
      clearTimeout(botTimerRef.current);
    }

    botTimerRef.current = setTimeout(() => {
      void action();
    }, 350);
  }, [game.handNumber, game.phase, handleReveal, handleShowdown, loading, playMode, wallet]);

  const formatElapsed = (s: number) => {
    const m = Math.floor(s / 60);
    const sec = s % 60;
    return m > 0 ? `${m}m ${sec}s` : `${sec}s`;
  };

  const dealerLine = (() => {
    if (loading) {
      const timer = ` [${formatElapsed(elapsed)}]`;
      switch (activeRequest) {
        case "deal":
          return `Dealer: SHUFFLING & GENERATING DEAL PROOF... (~30-60s)${timer}`;
        case "flop":
          return `Dealer: GENERATING REVEAL PROOF... (~20-40s)${timer}`;
        case "turn":
          return `Dealer: GENERATING REVEAL PROOF... (~20-40s)${timer}`;
        case "river":
          return `Dealer: GENERATING REVEAL PROOF... (~20-40s)${timer}`;
        case "showdown":
          return `Dealer: VERIFYING SHOWDOWN — THIS TAKES 2-4 MINUTES. PLEASE WAIT.${timer}`;
        default:
          return `Dealer: One moment...${timer}`;
      }
    }

    if (playMode === "single" && botLine && game.phase !== "waiting") {
      return `Dealer: ${botLine}`;
    }

    if (wallet && !isWalletSeated && seatedAddresses.length > 0) {
      return `Dealer: On-chain seats are ${tableSeatLabel}. Click DEAL CARDS to run a hand with your connected wallet.`;
    }

    switch (game.phase) {
      case "waiting":
        if (playMode === "single") {
          return "Dealer: Solo mode. Join table, then click DEAL CARDS.";
        }
        if (playMode === "headsup") {
          if ((lobby?.joined_wallets ?? 0) < 2) {
            return "Dealer: Two-player mode needs 2 joined wallets. Share table ID and wait for one join.";
          }
          return "Dealer: Heads-up is ready. Click DEAL CARDS.";
        }
        if ((lobby?.joined_wallets ?? 0) < 3) {
          return "Dealer: 3-6 player mode needs at least 3 joined wallets.";
        }
        return "Dealer: Multi-player table is ready. Click DEAL CARDS.";
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
          </div>

          <div className="flex items-center gap-3">
            <div className="text-[9px]" style={{ color: "#c8e6ff" }}>
              HAND #{game.handNumber} | {game.phase.toUpperCase()}
            </div>

            {game.lastTxHash ? (
              <a
                href={`https://stellar.expert/explorer/testnet/tx/${game.lastTxHash}`}
                target="_blank"
                rel="noopener noreferrer"
                className="text-[9px]"
                style={{ color: "#3498db", textDecoration: "none" }}
              >
                EXPLORER ↗
              </a>
            ) : (
              <a
                href="https://stellar.expert/explorer/testnet"
                target="_blank"
                rel="noopener noreferrer"
                className="text-[9px]"
                style={{ color: "#3498db", textDecoration: "none" }}
              >
                EXPLORER ↗
              </a>
            )}

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
                    isCurrentTurn={game.players[game.currentTurn]?.address === player.address}
                    isDealer={player.seat === game.dealerSeat}
                    isUser={false}
                    isWinner={!!winnerAddress && player.address === winnerAddress}
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
              <Board cards={game.boardCards} pot={game.pot} />

              {/* Phase action buttons */}
              <div className="flex gap-2 mt-1">
                {game.phase === "preflop" && (
                  <button
                    onClick={() => handleReveal("flop")}
                    disabled={loading || !wallet}
                    className="pixel-btn pixel-btn-dark text-[9px]"
                    style={{ padding: "6px 14px", opacity: loading || !wallet ? 0.7 : 1 }}
                  >
                    DEAL FLOP
                  </button>
                )}
                {game.phase === "flop" && (
                  <button
                    onClick={() => handleReveal("turn")}
                    disabled={loading || !wallet}
                    className="pixel-btn pixel-btn-dark text-[9px]"
                    style={{ padding: "6px 14px", opacity: loading || !wallet ? 0.7 : 1 }}
                  >
                    DEAL TURN
                  </button>
                )}
                {game.phase === "turn" && (
                  <button
                    onClick={() => handleReveal("river")}
                    disabled={loading || !wallet}
                    className="pixel-btn pixel-btn-dark text-[9px]"
                    style={{ padding: "6px 14px", opacity: loading || !wallet ? 0.7 : 1 }}
                  >
                    DEAL RIVER
                  </button>
                )}
                {(game.phase === "river" || game.phase === "showdown") && (
                  <button
                    onClick={handleShowdown}
                    disabled={loading || !wallet}
                    className="pixel-btn pixel-btn-gold text-[9px]"
                    style={{ padding: "6px 14px", opacity: loading || !wallet ? 0.7 : 1 }}
                  >
                    {game.phase === "showdown" ? "RESOLVE SHOWDOWN (2-4 MIN)" : "SHOWDOWN (2-4 MIN)"}
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
            canStartHand={canStartHand}
            canResolveShowdown={!!wallet}
            statusHint={seatStatusHint}
            loading={loading}
            isSolo={playMode === "single"}
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
            <span className="text-[8px]" style={{ color: "#7f8c8d" }}>
              MPC: 3/3 NODES | TACEO CO-NOIR REP3
            </span>
            {game.proofSize && (
              <span
                className="pixel-border-thin px-1 py-0.5 text-[8px]"
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
                  style={{ color: "#3498db" }}
                >
                  {game.lastTxHash.slice(0, 8)}...{game.lastTxHash.slice(-8)}
                </a>
              </span>
            </div>
          )}
        </div>

        <div className="fixed bottom-[14%] left-[5%] z-[5]">
          <PixelCat sprite={17} size={48} />
        </div>
        <div className="fixed bottom-[13%] right-[5%] z-[5]">
          <PixelCat sprite={21} size={56} flipped />
        </div>
      </div>
    </PixelWorld>
  );
}
