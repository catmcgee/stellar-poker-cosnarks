import type { GamePhase } from "@/lib/game-state";
import type { TableLobbyResponse } from "@/lib/api";

type ActiveRequest = "deal" | "flop" | "turn" | "river" | "showdown" | null;
type PlayMode = "single" | "headsup" | "multi";

function shortAddr(address: string): string {
  return `${address.slice(0, 6)}...${address.slice(-6)}`;
}

export function getDealerLine(opts: {
  loading: boolean;
  elapsed: number;
  activeRequest: ActiveRequest;
  playMode: PlayMode;
  botLine: string | null;
  onChainPhase: string;
  gamePhase: GamePhase;
  wallet: boolean;
  isWalletSeated: boolean;
  seatedAddresses: string[];
  tableSeatLabel: string;
  winnerAddress: string | null;
  userAddress: string | undefined;
  lobby: TableLobbyResponse | null;
}): string {
  const formatElapsed = (s: number) => {
    const m = Math.floor(s / 60);
    const sec = s % 60;
    return m > 0 ? `${m}m ${sec}s` : `${sec}s`;
  };

  if (opts.loading) {
    const timer = ` [${formatElapsed(opts.elapsed)}]`;
    switch (opts.activeRequest) {
      case "deal":
        return `SHUFFLING & GENERATING DEAL PROOF... (~30-60s)${timer}`;
      case "flop":
        return `GENERATING REVEAL PROOF... (~20-40s)${timer}`;
      case "turn":
        return `GENERATING REVEAL PROOF... (~20-40s)${timer}`;
      case "river":
        return `GENERATING REVEAL PROOF... (~20-40s)${timer}`;
      case "showdown":
        return `VERIFYING SHOWDOWN — THIS TAKES 2-4 MINUTES. PLEASE WAIT.${timer}`;
      default:
        return `One moment...${timer}`;
    }
  }

  if (opts.playMode === "single" && opts.botLine && opts.gamePhase !== "waiting") {
    return `${opts.botLine}`;
  }

  if (opts.onChainPhase === "DealingFlop") {
    return "Betting round complete. Dealer is revealing the flop...";
  }
  if (opts.onChainPhase === "DealingTurn") {
    return "Betting round complete. Dealer is revealing the turn...";
  }
  if (opts.onChainPhase === "DealingRiver") {
    return "Betting round complete. Dealer is revealing the river...";
  }
  if (opts.onChainPhase === "Showdown") {
    return "Betting complete. Dealer is resolving showdown...";
  }

  if (opts.playMode !== "single" && opts.wallet && !opts.isWalletSeated && opts.seatedAddresses.length > 0) {
    return `On-chain seats are ${opts.tableSeatLabel}. Click JOIN TABLE to take a seat with this wallet.`;
  }

  switch (opts.gamePhase) {
    case "waiting":
      if (opts.playMode === "single") {
        return "Solo vs AI uses fake chips (100 each). Click DEAL CARDS to start.";
      }
      if (opts.playMode === "headsup") {
        if ((opts.lobby?.joined_wallets ?? 0) < 2) {
          return "Two-player mode needs 2 joined wallets. Share table ID and wait for one join.";
        }
        return "Heads-up is ready. Click DEAL CARDS.";
      }
      if ((opts.lobby?.joined_wallets ?? 0) < 3) {
        return "3-6 player mode needs at least 3 joined wallets.";
      }
      return "Multi-player table is ready. Click DEAL CARDS.";
    case "dealing":
      return "Cards are being dealt.";
    case "preflop":
      return "Preflop is live. Place your bet; dealer auto-reveals next street.";
    case "flop":
      return "Flop is out. Place your bet; dealer auto-reveals turn.";
    case "turn":
      return "Turn is out. Place your bet; dealer auto-reveals river.";
    case "river":
      return "River is out. Final betting round; dealer auto-runs showdown.";
    case "showdown":
      return "Showdown in progress.";
    case "settlement":
      if (opts.winnerAddress) {
        if (opts.userAddress && opts.winnerAddress === opts.userAddress) {
          return "Hand complete. YOU WIN!";
        }
        if (opts.playMode === "single" && opts.userAddress && opts.winnerAddress !== opts.userAddress) {
          return "Hand complete. AI WINS!";
        }
        return `Hand complete. Winner: ${shortAddr(opts.winnerAddress)}.`;
      }
      return "Hand complete. Start the next hand when ready.";
    default:
      return "Ready when you are.";
  }
}
