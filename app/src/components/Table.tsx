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
import { connectFreighterWallet, type WalletSession } from "@/lib/freighter";

interface TableProps {
  tableId: number;
}

type ActiveRequest = "deal" | "flop" | "turn" | "river" | "showdown" | null;
type PlayMode = "single" | "headsup" | "multi";

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
  const [multiOpponents, setMultiOpponents] = useState<string[]>(["", ""]);
  const [playMode, setPlayMode] = useState<PlayMode>("single");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [connectingWallet, setConnectingWallet] = useState(false);
  const [activeRequest, setActiveRequest] = useState<ActiveRequest>(null);
  const [onChainPhase, setOnChainPhase] = useState<string>("unknown");
  const [winnerAddress, setWinnerAddress] = useState<string | null>(null);
  const [botLine, setBotLine] = useState<string | null>(null);
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
                const address = preserveLocalSeatAddresses
                  ? prev.players[index]?.address ?? chainAddress
                  : aliasWalletSeatForLocalDev && index === 0
                    ? userAddress
                  : chainAddress;
                const existing =
                  prev.players.find((p) => p.address === address) ?? prev.players[index];
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
      if (multiOpponents.every((entry) => !entry.trim()) && game.players.length >= 2) {
        const known = game.players
          .filter((p) => p.address !== connected.address)
          .map((p) => p.address)
          .slice(0, 5);
        if (known.length > 0) {
          while (known.length < 2) {
            known.push("");
          }
          setMultiOpponents(known);
        }
      }
      await hydrateMyCards(connected);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to connect wallet");
    } finally {
      setConnectingWallet(false);
    }
  }, [game.players, hydrateMyCards, multiOpponents, opponentAddress]);

  const resolvePlayersForDeal = useCallback((): string[] | null => {
    if (!wallet) {
      setError("Connect Freighter wallet before starting a hand");
      return null;
    }

    if (!isStellarAddress(wallet.address)) {
      setError("Connected wallet address is invalid");
      return null;
    }

    const existingOpponents = game.players
      .filter((p) => p.address !== wallet.address)
      .map((p) => p.address);
    if (playMode === "single") {
      const opponent = (
        existingOpponents[0] ??
        opponentAddress ??
        process.env.NEXT_PUBLIC_SINGLE_PLAYER_OPPONENT ??
        ""
      ).trim();
      if (!isStellarAddress(opponent)) {
        setError("Solo mode could not find a valid local opponent seat");
        return null;
      }
      if (opponent === wallet.address) {
        setError("Opponent seat cannot match your wallet");
        return null;
      }
      return [wallet.address, opponent];
    }

    if (playMode === "headsup") {
      const opponent = (existingOpponents[0] ?? opponentAddress).trim();
      if (!isStellarAddress(opponent)) {
        setError("Enter a valid opponent Stellar address");
        return null;
      }
      if (opponent === wallet.address) {
        setError("Opponent address must be different from your wallet");
        return null;
      }
      return [wallet.address, opponent];
    }

    const submittedOpponents = multiOpponents
      .map((address) => address.trim())
      .filter((address) => address.length > 0);
    const opponents = submittedOpponents.length > 0 ? submittedOpponents : existingOpponents;

    if (opponents.length < 2) {
      setError("Multi-player mode needs at least 2 opponents (3 total players)");
      return null;
    }
    if (opponents.length > 5) {
      setError("Multi-player mode supports up to 6 total players");
      return null;
    }

    const players = [wallet.address, ...opponents];
    for (const address of players) {
      if (!isStellarAddress(address)) {
        setError(`Invalid Stellar address in player list: ${address}`);
        return null;
      }
    }

    const unique = new Set(players);
    if (unique.size !== players.length) {
      setError("Duplicate player addresses are not allowed");
      return null;
    }

    return players;
  }, [wallet, game.players, opponentAddress, playMode, multiOpponents]);

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
      ? "Connected wallet is not in on-chain seats. You can still click DEAL to run a hand with this wallet."
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
    }, 900);
  }, [game.handNumber, game.phase, handleReveal, handleShowdown, loading, playMode, wallet]);

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

    if (playMode === "single" && botLine && game.phase !== "waiting") {
      return `Dealer: ${botLine}`;
    }

    if (wallet && !isWalletSeated && seatedAddresses.length > 0) {
      return `Dealer: On-chain seats are ${tableSeatLabel}. Click DEAL CARDS to run a hand with your connected wallet.`;
    }

    switch (game.phase) {
      case "waiting":
        if (playMode === "single") {
          return "Dealer: Solo mode. Opponent seat is automatic, click DEAL CARDS.";
        }
        if (playMode === "headsup") {
          return "Dealer: Two-player mode. Enter opponent wallet, then click DEAL CARDS.";
        }
        return "Dealer: Multi-player mode. Enter 3-6 total player wallets, then click DEAL CARDS.";
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

  const setMultiOpponentAt = (index: number, value: string) => {
    setMultiOpponents((prev) => {
      const next = [...prev];
      next[index] = value;
      return next;
    });
  };

  const addMultiOpponentField = () => {
    setMultiOpponents((prev) => (prev.length >= 5 ? prev : [...prev, ""]));
  };

  const removeMultiOpponentField = (index: number) => {
    setMultiOpponents((prev) => {
      if (prev.length <= 2) return prev;
      return prev.filter((_, i) => i !== index);
    });
  };

  return (
    <PixelWorld autoNight>
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
            <PixelChip color="red" size={3} />
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

        {/* Mode + opponent controls */}
        {game.phase === "waiting" && (
          <div className="w-full max-w-3xl flex flex-col gap-2">
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={() => setPlayMode("single")}
                className="pixel-btn text-[7px]"
                style={{
                  padding: "4px 10px",
                  opacity: playMode === "single" ? 1 : 0.7,
                  background: playMode === "single" ? "#145a32" : "#2c3e50",
                }}
              >
                1 PLAYER
              </button>
              <button
                type="button"
                onClick={() => setPlayMode("headsup")}
                className="pixel-btn text-[7px]"
                style={{
                  padding: "4px 10px",
                  opacity: playMode === "headsup" ? 1 : 0.7,
                  background: playMode === "headsup" ? "#7d6608" : "#2c3e50",
                }}
              >
                2 PLAYER
              </button>
              <button
                type="button"
                onClick={() => setPlayMode("multi")}
                className="pixel-btn text-[7px]"
                style={{
                  padding: "4px 10px",
                  opacity: playMode === "multi" ? 1 : 0.7,
                  background: playMode === "multi" ? "#1f618d" : "#2c3e50",
                }}
              >
                3-6 PLAYERS
              </button>
            </div>

            {playMode === "headsup" ? (
              <input
                type="text"
                value={opponentAddress}
                onChange={(e) => setOpponentAddress(e.target.value.trim())}
                placeholder="OPPONENT ADDRESS (G...)"
                className="flex-1 text-[7px]"
                style={{ padding: "6px 10px" }}
              />
            ) : playMode === "multi" ? (
              <div className="flex flex-col gap-2">
                {multiOpponents.map((address, index) => (
                  <div key={`multi-seat-${index}`} className="flex items-center gap-2">
                    <input
                      type="text"
                      value={address}
                      onChange={(e) => setMultiOpponentAt(index, e.target.value.trim())}
                      placeholder={`PLAYER ${index + 2} ADDRESS (G...)`}
                      className="flex-1 text-[7px]"
                      style={{ padding: "6px 10px" }}
                    />
                    {multiOpponents.length > 2 && (
                      <button
                        type="button"
                        onClick={() => removeMultiOpponentField(index)}
                        className="pixel-btn text-[7px]"
                        style={{ padding: "4px 8px", background: "#7b241c" }}
                      >
                        -
                      </button>
                    )}
                  </div>
                ))}
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={addMultiOpponentField}
                    disabled={multiOpponents.length >= 5}
                    className="pixel-btn text-[7px]"
                    style={{
                      padding: "4px 10px",
                      opacity: multiOpponents.length >= 5 ? 0.6 : 1,
                      background: "#1f618d",
                    }}
                  >
                    + ADD PLAYER
                  </button>
                  <span className="text-[7px]" style={{ color: "#c8e6ff" }}>
                    TOTAL PLAYERS: {1 + multiOpponents.filter((a) => a.trim().length > 0).length} / 6
                  </span>
                </div>
              </div>
            ) : (
              <div
                className="pixel-border-thin px-3 py-2 text-[7px]"
                style={{
                  color: "#c8e6ff",
                  background: "rgba(10, 20, 30, 0.55)",
                  borderColor: "rgba(140, 170, 200, 0.45)",
                }}
              >
                SOLO MODE: Opponent seat auto-selected from table seats.
              </div>
            )}
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

        <div
          className="w-full max-w-3xl pixel-border-thin px-4 py-2"
          style={{
            background: "rgba(10, 20, 30, 0.55)",
            borderColor: "rgba(140, 170, 200, 0.45)",
          }}
        >
          <span className="text-[7px]" style={{ color: "#c8e6ff" }}>
            TABLE SEATS: {tableSeatLabel}
            {wallet
              ? isWalletSeated
                ? ` | YOU: ${shortAddress(wallet.address)}`
                : ` | CONNECTED: ${shortAddress(wallet.address)} (NOT SEATED)`
              : ""}
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
                    disabled={loading || !wallet}
                    className="pixel-btn pixel-btn-dark text-[7px]"
                    style={{ padding: "4px 12px", opacity: loading || !wallet ? 0.7 : 1 }}
                  >
                    DEAL FLOP
                  </button>
                )}
                {game.phase === "flop" && (
                  <button
                    onClick={() => handleReveal("turn")}
                    disabled={loading || !wallet}
                    className="pixel-btn pixel-btn-dark text-[7px]"
                    style={{ padding: "4px 12px", opacity: loading || !wallet ? 0.7 : 1 }}
                  >
                    DEAL TURN
                  </button>
                )}
                {game.phase === "turn" && (
                  <button
                    onClick={() => handleReveal("river")}
                    disabled={loading || !wallet}
                    className="pixel-btn pixel-btn-dark text-[7px]"
                    style={{ padding: "4px 12px", opacity: loading || !wallet ? 0.7 : 1 }}
                  >
                    DEAL RIVER
                  </button>
                )}
                {(game.phase === "river" || game.phase === "showdown") && (
                  <button
                    onClick={handleShowdown}
                    disabled={loading || !wallet}
                    className="pixel-btn pixel-btn-gold text-[7px]"
                    style={{ padding: "4px 12px", opacity: loading || !wallet ? 0.7 : 1 }}
                  >
                    {game.phase === "showdown" ? "RESOLVE SHOWDOWN" : "SHOWDOWN"}
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
            canStartHand={canStartHand}
            canResolveShowdown={!!wallet}
            statusHint={seatStatusHint}
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
                <PixelChip color="gold" size={2} />
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
