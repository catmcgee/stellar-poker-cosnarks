export type GamePhase =
  | "waiting"
  | "dealing"
  | "preflop"
  | "flop"
  | "turn"
  | "river"
  | "showdown"
  | "settlement";

export interface Player {
  address: string;
  seat: number;
  stack: number;
  betThisRound: number;
  folded: boolean;
  allIn: boolean;
  cards?: [number, number];
}

export interface GameState {
  tableId: number;
  phase: GamePhase;
  players: Player[];
  pot: number;
  boardCards: number[];
  currentTurn: number;
  dealerSeat: number;
  handNumber: number;
  lastTxHash?: string;
  proofSize?: number;
  onChainConfirmed: boolean;
}

export function createInitialState(tableId: number): GameState {
  return {
    tableId,
    phase: "waiting",
    players: [],
    pot: 0,
    boardCards: [],
    currentTurn: 0,
    dealerSeat: 0,
    handNumber: 0,
    onChainConfirmed: false,
  };
}
