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

export interface AuthSigner {
  address: string;
  signMessage: (message: string) => Promise<string>;
}

let lastNonce = 0;

function nextNonce(): string {
  const now = Date.now() * 1000;
  if (now > lastNonce) {
    lastNonce = now;
  } else {
    lastNonce += 1;
  }
  return String(lastNonce);
}

function buildAuthMessage(
  address: string,
  tableId: number,
  action: string,
  nonce: string,
  timestamp: number
): string {
  return `stellar-poker|${address}|${tableId}|${action}|${nonce}|${timestamp}`;
}

async function buildAuthHeaders(
  tableId: number,
  action: string,
  auth: AuthSigner
): Promise<Record<string, string>> {
  const nonce = nextNonce();
  const timestamp = Math.floor(Date.now() / 1000);
  const message = buildAuthMessage(auth.address, tableId, action, nonce, timestamp);
  const signature = await auth.signMessage(message);

  return {
    "x-player-address": auth.address,
    "x-auth-signature": signature,
    "x-auth-nonce": nonce,
    "x-auth-timestamp": String(timestamp),
  };
}

export async function requestDeal(
  tableId: number,
  players: string[],
  auth: AuthSigner
): Promise<DealResponse> {
  const headers = await buildAuthHeaders(tableId, "request_deal", auth);

  const res = await fetch(`${API_BASE}/api/table/${tableId}/request-deal`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      ...headers,
    },
    body: JSON.stringify({ players }),
  });
  if (!res.ok) throw new Error(`Deal failed: ${res.status}`);
  return res.json();
}

export async function requestReveal(
  tableId: number,
  phase: "flop" | "turn" | "river",
  auth: AuthSigner
): Promise<RevealResponse> {
  const headers = await buildAuthHeaders(
    tableId,
    `request_reveal:${phase}`,
    auth
  );

  const res = await fetch(`${API_BASE}/api/table/${tableId}/request-reveal/${phase}`, {
    method: "POST",
    headers,
  });
  if (!res.ok) throw new Error(`Reveal failed: ${res.status}`);
  return res.json();
}

export async function requestShowdown(
  tableId: number,
  auth: AuthSigner
): Promise<ShowdownResponse> {
  const headers = await buildAuthHeaders(tableId, "request_showdown", auth);

  const res = await fetch(`${API_BASE}/api/table/${tableId}/request-showdown`, {
    method: "POST",
    headers,
  });
  if (!res.ok) throw new Error(`Showdown failed: ${res.status}`);
  return res.json();
}

export async function getPlayerCards(
  tableId: number,
  address: string,
  auth: AuthSigner
): Promise<PlayerCardsResponse> {
  const headers = await buildAuthHeaders(tableId, "get_player_cards", auth);

  const res = await fetch(`${API_BASE}/api/table/${tableId}/player/${address}/cards`, {
    headers,
  });
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
