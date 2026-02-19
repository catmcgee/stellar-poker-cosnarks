const SUITS = ["clubs", "diamonds", "hearts", "spades"] as const;
const RANKS = [
  "2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K", "A",
] as const;

export type Suit = (typeof SUITS)[number];
export type Rank = (typeof RANKS)[number];

export interface CardInfo {
  value: number;
  suit: Suit;
  rank: Rank;
  display: string;
  suitSymbol: string;
  color: "red" | "black";
}

export function decodeCard(value: number): CardInfo {
  const suit = SUITS[Math.floor(value / 13)];
  const rank = RANKS[value % 13];
  const suitSymbols: Record<Suit, string> = {
    clubs: "\u2663",
    diamonds: "\u2666",
    hearts: "\u2665",
    spades: "\u2660",
  };
  const color = suit === "hearts" || suit === "diamonds" ? "red" : "black";
  return {
    value,
    suit,
    rank,
    display: `${rank}${suitSymbols[suit]}`,
    suitSymbol: suitSymbols[suit],
    color,
  };
}

export function cardBack(): string {
  return "?";
}
