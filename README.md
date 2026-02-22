# Stellar Poker

Onchain Texas Hold'em poker on Stellar with private cards using cosnarks (ZKMPC).

No single party ever sees your cards. A committee of MPC nodes (running TACEO coNoir) shuffles and deals using REP3 secret sharing. UltraHonk ZK proofs verify every deal, reveal, and showdown onchain.

This was developed for the [Stellar ZK Hackathon](https://dorahacks.io/hackathon/stellar-hacks-zk-gaming) and utilizes the [Stellar game studio](https://jamesbachini.github.io/Stellar-Game-Studio/) and the [Ultrahonk soroban verifier](https://github.com/indextree/ultrahonk_soroban_contract). As a ZK nerd new to gaming, I had a lot of fun building this and you'll be happy to hear it's not AI slop :)

If you are new to MPC or do not fully grasp the limiations of ZK by itself, please check out the [slide deck](https://www.canva.com/design/DAHB5JrdEAk/XThK1QgbEATHwZ0rX-W2aA/view?utm_content=DAHB5JrdEAk&utm_campaign=designshare&utm_medium=link2&utm_source=uniquelinks&utlId=hb4aca74548) which explains why any game that relies on card-shuffling between multiple players cannot use ZK alone.

I've also written a reusable crate that others can use to do card-shuffling in their Soroban app.

## How it works

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
       [Soroban]           <-- Onchain settlement
    /      |       \
[PokerTable] [ZKVerifier] [CommitteeRegistry]
```

Multiplayer can be up to 6 players. There is also a Solo mode against an AI (the old definition of AI, ie a bunch of if statements and some randomness).

### Key Properties

- **Private cards**: This is the coolest part. No single party (not even the coordinator) sees the full deck. Cards exist only as REP3 secret shares across 3 MPC nodes. ZK cannot handle this alone.
- **ZK-verified**: Deal, reveal, and showdown proofs are UltraHonk proofs verified onchain via Soroban's native BN254 host functions (Protocol 25).
- **Trustless settlement**: All bets, pot calculation, and payouts happen in Soroban smart contracts. Game logic is handled here!
- **Honest majority**: As long as there is an honest majority of nodes in TACEO (in our case 2 nodes), privacy will be maintained.

## Repository Structure

```
stellar-poker/
  contracts/
    poker-table/        -- Main game contract (betting, state machine, settlement)
    zk-verifier/        -- UltraHonk proof verification (BN254 native ops)
    committee-registry/ -- MPC committee management and slashing
    game-hub/           -- Mock Game Hub contract (Stellar Game Studio interface)
  circuits/
    lib/                -- Shared Noir library (cards, commitments, Merkle)
    deal_valid/         -- Proves deck shuffle + deal consistency
    reveal_board_valid/ -- Proves community card reveals match committed deck
    showdown_valid/     -- Proves winner has the best hand
  stellar-zk-cards/    -- Reusable card game library (encoding, hand eval)
  services/
    coordinator/        -- Axum HTTP server orchestrating MPC sessions
    node/               -- MPC node (TACEO coNoir participant)
  app/                  -- Next.js web frontend
  tests/                -- Integration and property-based tests
  vendor/               -- Vendored UltraHonk verifier dependencies
  crs/                  -- BN254 common reference string data
  scripts/              -- Deploy and setup scripts
  docker-compose.yml    -- Full stack local development
```

## Tech Stack

| Component       | Technology                               |
| --------------- | ---------------------------------------- |
| Smart contracts | Soroban (Rust, soroban-sdk 22.0.0)       |
| ZK proofs       | Noir circuits + UltraHonk (Barretenberg) |
| MPC             | TACEO coNoir (REP3, 3-party)             |

We use Poseidon2 for hashing.

## Prerequisites

- Rust
- Nargo 1.0.0-beta.17 (Noir compiler)
- Node.js 18+
- Docker (needed for local soroban and MPC)
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
cargo build
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

1. **Create table**: Admin creates a `PokerTable` contract with config (blinds, buy-in range, timeout)
2. **Join**: Players join with a buy-in (tokens escrowed in contract)
3. **Start hand**: Triggers the MPC committee to shuffle and deal
4. **Deal**: Committee generates a `deal_valid` ZK proof, commits deck Merkle root + hand commitments on-chain, privately delivers hole cards to each player
5. **Betting**: Players submit actions (fold/check/call/bet/raise/all-in) to the contract
6. **Reveal**: After each betting round, committee reveals community cards with `reveal_board_valid` proof
7. **Showdown**: Committee reveals remaining hands, generates `showdown_valid` proof, contract settles pot and winner can claim onchain

## Circuits

Circuits are written in Noir! They are proved inside TACEO MPC network CoNoir.

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

## Difficulties

This was my first time building on Stellar. It was mostly seamless especially with the help of AI tools, but AI really sucks when it comes to privacy. So I wrote down some issues that I ran into in [DIFFICULTIES.md](/DIFFICULTIES.md).
