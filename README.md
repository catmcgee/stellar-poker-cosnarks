# Stellar Poker

Onchain Texas Hold'em poker on Stellar (Soroban) with private cards using MPC + ZK proofs.

No single party ever sees your cards. A committee of 3 MPC nodes (running TACEO coNoir) shuffles and deals using REP3 secret sharing. UltraHonk ZK proofs verify every deal, reveal, and showdown on-chain.

## Architecture

```
Player A          Player B
   |                  |
   +------+  +-------+
          |  |
       [Web App]          <-- Next.js frontend
          |
       [Coordinator]      <-- Orchestrates MPC sessions
       /    |    \
   [Node0] [Node1] [Node2]   <-- TACEO coNoir MPC nodes (REP3)
          |
       [Soroban]           <-- On-chain settlement
    /      |       \
[PokerTable] [ZKVerifier] [CommitteeRegistry]
```

### Key Properties

- **Private cards**: No single party (not even the coordinator) sees the full deck. Cards exist only as REP3 secret shares across 3 MPC nodes.
- **ZK-verified**: Deal, reveal, and showdown proofs are standard UltraHonk proofs verified on-chain via Soroban's native BN254 host functions (Protocol 25).
- **Trustless settlement**: All bets, pot calculation, and payouts happen in Soroban smart contracts.
- **Honest majority**: Privacy holds with up to 1 malicious MPC node. 2+ colluding nodes can reconstruct secrets but are detectable via audit logs and on-chain slashing.

## Repository Structure

```
stellar-poker/
  contracts/
    poker-table/      -- Main game contract (betting, state machine, settlement)
    zk-verifier/      -- UltraHonk proof verification (BN254 native ops)
    committee-registry/ -- MPC committee management and slashing
  circuits/
    lib/               -- Shared Noir library (cards, commitments, Merkle)
    deal_valid/        -- Proves deck shuffle + deal consistency
    reveal_board_valid/ -- Proves community card reveals match committed deck
    showdown_valid/    -- Proves winner has the best hand
  stellar-zk-cards/   -- Reusable card game library (encoding, hand eval)
  services/
    coordinator/       -- Axum HTTP server orchestrating MPC sessions
    node/              -- MPC node (TACEO coNoir participant)
  app/                 -- Next.js web frontend
  scripts/             -- Deploy and setup scripts
  docker-compose.yml   -- Full stack local development
```

## Tech Stack

| Component | Technology |
|-----------|-----------|
| Smart contracts | Soroban (Rust, soroban-sdk 22.0.0) |
| ZK proofs | Noir circuits + UltraHonk (Barretenberg) |
| MPC | TACEO coNoir (REP3, 3-party) |
| On-chain verification | Native BN254 host functions (Protocol 25 / X-Ray) |
| Commitments | Poseidon2 over BN254 |
| Coordinator | Rust (Axum) |
| Frontend | Next.js + TypeScript + Tailwind CSS |
| Wallet | Freighter (Stellar) |

## Prerequisites

- Rust + `wasm32-unknown-unknown` target
- Nargo 1.0.0-beta.17 (Noir compiler)
- Node.js 18+
- Docker (for local Soroban and MPC nodes)
- Stellar CLI (`cargo install stellar-cli --features opt`)

## Quick Start

```bash
# Setup (installs deps, builds everything)
./scripts/setup.sh

# Start all services
docker-compose up

# Or run individually:
cargo run -p coordinator     # Port 8080
cd app && npm run dev        # Port 3000
```

## Development

### Build contracts
```bash
cargo check                  # Check all crates
cargo build --release --target wasm32-unknown-unknown -p poker-table
```

### Build/test circuits
```bash
./scripts/compile-circuits.sh
cd circuits/lib && nargo test
```

### Run web app
```bash
cd app && npm run dev
```

### Deploy to testnet
```bash
NETWORK=testnet ./scripts/deploy.sh
```

## Game Flow

1. **Create table**: Admin creates a PokerTable contract with config (blinds, buy-in range, timeout)
2. **Join**: Players join with a buy-in (tokens escrowed in contract)
3. **Start hand**: Triggers the MPC committee to shuffle and deal
4. **Deal**: Committee generates a `deal_valid` ZK proof, commits deck Merkle root + hand commitments on-chain, privately delivers hole cards to each player
5. **Betting**: Players submit actions (fold/check/call/bet/raise/all-in) to the contract
6. **Reveal**: After each betting round, committee reveals community cards with `reveal_board_valid` proof
7. **Showdown**: Committee reveals remaining hands, generates `showdown_valid` proof, contract settles pot

## Circuit Design

### deal_valid
- **Private inputs**: deck[52], salts[52] (secret-shared in MPC)
- **Public inputs**: deck_root, hand_commitments[6], dealt_indices
- **Proves**: Valid 52-card deck, Merkle root matches commitments, hand commitments match dealt cards

### reveal_board_valid
- **Private inputs**: deck[52], salts[52]
- **Public inputs**: deck_root, revealed_cards, revealed_indices, previously_used_indices
- **Proves**: Revealed cards match committed deck, no indices reused

### showdown_valid
- **Private inputs**: hole_cards, board_cards, salts
- **Public inputs**: hand_commitments, board_commitments, declared_winner
- **Proves**: Cards match commitments, hand evaluation is correct, winner has best hand

## Security Model

- **MPC trust**: Honest majority (2-of-3). Privacy holds with 1 corruption.
- **ZK soundness**: UltraHonk proofs are computationally sound. A cheating committee cannot forge proofs.
- **On-chain finality**: Soroban contracts are the source of truth for game state and pot settlement.
- **Timeout protection**: If a player or the committee stalls, anyone can trigger a timeout. Player timeout = auto-fold. Committee timeout = dispute + emergency refund.
- **Slashing**: Committee members stake tokens. Misbehavior reports trigger slashing (50% after 3 reports).

## License

MIT
