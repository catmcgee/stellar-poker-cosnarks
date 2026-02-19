const API_BASE = process.env.NEXT_PUBLIC_COORDINATOR_URL || "http://localhost:8080";

export interface DealResponse {
  status: string;
  deck_root: string;
  hand_commitments: string[];
  proof_size: number;
  session_id: string;
  tx_hash: string | null;
}

export interface RevealResponse {
  status: string;
  cards: number[];
  proof_size: number;
  session_id: string;
  tx_hash: string | null;
}

export interface ShowdownResponse {
  status: string;
  winner: string;
  winner_index: number;
  proof_size: number;
  session_id: string;
  tx_hash: string | null;
}

export interface TableStateResponse {
  state: string;
}

export interface PlayerCardsResponse {
  card1: number;
  card2: number;
  salt1: string;
  salt2: string;
}

export interface CommitteeStatusResponse {
  nodes: number;
  healthy: boolean[];
  status: string;
}

export async function requestDeal(tableId: number): Promise<DealResponse> {
  const res = await fetch(`${API_BASE}/api/table/${tableId}/request-deal`, {
    method: "POST",
  });
  if (!res.ok) throw new Error(`Deal failed: ${res.status}`);
  return res.json();
}

export async function requestReveal(
  tableId: number,
  phase: "flop" | "turn" | "river"
): Promise<RevealResponse> {
  const res = await fetch(
    `${API_BASE}/api/table/${tableId}/request-reveal/${phase}`,
    { method: "POST" }
  );
  if (!res.ok) throw new Error(`Reveal failed: ${res.status}`);
  return res.json();
}

export async function requestShowdown(
  tableId: number
): Promise<ShowdownResponse> {
  const res = await fetch(
    `${API_BASE}/api/table/${tableId}/request-showdown`,
    { method: "POST" }
  );
  if (!res.ok) throw new Error(`Showdown failed: ${res.status}`);
  return res.json();
}

export async function getPlayerCards(
  tableId: number,
  address: string
): Promise<PlayerCardsResponse> {
  const res = await fetch(
    `${API_BASE}/api/table/${tableId}/player/${address}/cards`
  );
  if (!res.ok) throw new Error(`Failed to get cards: ${res.status}`);
  return res.json();
}

export async function getTableState(
  tableId: number
): Promise<TableStateResponse> {
  const res = await fetch(`${API_BASE}/api/table/${tableId}/state`);
  if (!res.ok) throw new Error(`Failed to get table state: ${res.status}`);
  return res.json();
}

export async function getCommitteeStatus(): Promise<CommitteeStatusResponse> {
  const res = await fetch(`${API_BASE}/api/committee/status`);
  if (!res.ok) throw new Error(`Failed to get status: ${res.status}`);
  return res.json();
}
