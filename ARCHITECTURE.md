# Stellar Poker — Architecture & Protocol Design

## Section A: Holes / Risks

### A1. CRITICAL: UltraHonk Verification Cost on Soroban

**The Problem:** Soroban's per-transaction CPU limit is **100M instructions**. A single BN254 pairing in WASM costs ~560M instructions (5.6x over budget). UltraHonk verification requires multi-scalar multiplication (~40+ G1 scalar muls + additions) PLUS a pairing check (2 pairings).

**Mitigating Factor:** Protocol 25 (X-Ray, live Jan 22 2026) introduced **native BN254 host functions** (`bn254_g1_add`, `bn254_g1_mul`, `bn254_multi_pairing_check`) via CAP-0074. These execute as native code, not WASM. The metered cost for native operations is dramatically lower than in-WASM computation.

**Remaining Risk:** The exact metered cost of native BN254 operations hasn't been publicly benchmarked. The existing UltraHonk Soroban verifier (indextree/ultrahonk_soroban_contract) uses these host functions, but no public benchmark confirms it fits within 100M CPU instructions on mainnet.

**Validation Plan:**
1. Deploy the existing UltraHonk verifier contract on Soroban testnet
2. Submit a minimal proof and measure actual CPU instructions consumed
3. If over budget: split verification across multiple transactions (commit proof hash in tx1, verify in tx2+tx3 using checkpoint pattern) or use Groth16 (cheaper to verify but requires trusted setup)

### A2. CRITICAL: Proof Size vs Transaction Size Limits

**The Problem:** UltraHonk proofs are **~14.5 KB** + 32 bytes per public input. Soroban transactions are limited to **132 KB** and events+return value to **16 KB**. The proof itself fits, but combined with verification key (~3.5 KB), public inputs, and contract call overhead, it's tight.

**Risk:** If we have many public inputs (committed deck state, hand commitments, etc.), we could exceed transaction size limits.

**Mitigation:** Store the VK on-chain once (it's static per circuit). Pass proof + public inputs only. Keep public inputs minimal — use Poseidon hashes to compress state into single field elements.

### A3. HIGH: MPC Proof Generation Latency

**The Problem:** coNoir uses the REP3 protocol (3-party replicated secret sharing). For circuits of size 2^18, proof generation takes ~2 seconds on TACEO:Proof infrastructure. But MPC adds communication overhead — each multiplication gate requires network round-trips between 3 parties.

**Impact on UX:** A poker hand has multiple proof-generating moments (deal, flop, turn, river, showdown). If each takes 2-5 seconds for MPC proof generation + ~5 seconds for on-chain confirmation, a single hand could take 30-60+ seconds of overhead. Traditional online poker takes ~60-90 seconds per hand — we're consuming the entire budget on crypto overhead alone.

**Mitigation:**
- Minimize circuit sizes aggressively
- Pre-compute what we can (VK deployment, commitment generation)
- Pipeline: generate next proof while current betting round proceeds
- Accept that this is slower than traditional poker — market it as "provably fair"

### A4. HIGH: Full Shuffle Proof is Impractical in MPC

**The Problem:** The original spec asks for `deal_valid` proving shuffle correctness. A full shuffle proof for 52 cards using Bayer-Groth is ~50ms locally but ~87K R1CS constraints (zkShuffle numbers). Running this inside 3-party MPC would be dramatically slower — potentially 10-30+ seconds.

**The Real Issue:** In the Barnett-Smart protocol, EACH PLAYER must shuffle and re-encrypt the deck, producing a shuffle proof. With 6 players, that's 6 sequential shuffle proofs. This is a non-starter for interactive play.

**Revised Approach:** Use a **committee-trusted shuffle** with reveal proofs:
- Committee collectively shuffles the deck inside MPC (no proof of shuffle needed — the MPC itself guarantees correctness if honest majority holds)
- Commit to the shuffled deck as a Merkle root of Poseidon hashes
- Prove only **reveal consistency**: each dealt/revealed card is in the committed deck and hasn't been dealt before
- Prove **hand evaluation** at showdown

This trades a cryptographic shuffle guarantee for a committee trust assumption (honest majority of 2-of-3), but makes the system actually usable.

### A5. HIGH: Committee Liveness is a Single Point of Failure

**The Problem:** The MPC committee must be online for every deal, reveal, and proof generation. If 1 of 3 nodes goes down (in REP3, you need all 3 for correctness), the game halts.

**Mitigation:**
- Timeout mechanism: if committee doesn't respond within T seconds, players can claim refund on-chain
- Committee bond: staked funds slashed if they cause timeout
- Committee rotation: epoch-based, with handoff protocol
- For v1: accept 3-node committee with monitoring + auto-restart

### A6. MEDIUM: Events + Return Value Size Limit (16 KB)

**The Problem:** Soroban limits events + return values to **16 KB per transaction**. If we need to emit commitments, proof hashes, and game state updates, we're constrained.

**Mitigation:** Emit minimal events (hashes only). Store full state in contract storage. Clients reconstruct state from storage reads (which have higher limits).

### A7. MEDIUM: Card Encoding on BN254

**The Problem:** Cards need to be elements of the BN254 scalar field for Poseidon hashing. Simple encoding (0-51 as field elements) works for commitments but doesn't work for ElGamal-style encryption (needs curve points).

**Resolution:** Since we're using committee-based MPC (not player-by-player Barnett-Smart), we DON'T need ElGamal encryption of cards. Cards are plaintext field elements inside MPC. Only the commitments go on-chain. This simplifies encoding dramatically:
- Card = `suit * 13 + rank` (0-51), directly used as BN254 scalar field element
- Commitment = `Poseidon2(card, salt)` where salt is per-card randomness from MPC

### A8. MEDIUM: Privacy-Preserving Showdown Complexity

**The Problem:** Proving "my hand beats yours" in ZK without revealing hole cards requires a hand ranking circuit. Texas Hold'em evaluation (best 5 of 7 cards) is non-trivial: 21 combinations × full hand ranking logic. Estimated circuit size: 5K-20K constraints depending on implementation.

**Decision:** For v1, use **public showdown** (reveal hole cards at showdown, verify on-chain). Privacy-preserving showdown is a v2 feature. Rationale: In real poker, hole cards are revealed at showdown anyway. The privacy benefit is only for the losing player hiding their strategy — nice to have, not essential.

### A9. LOW: Contract Code Size (64-128 KB Limit)

**The Problem:** Soroban contracts must fit within the ledger entry size limit (128 KiB). The UltraHonk verifier uses aggressive optimization (`opt-level = z, lto = true, panic = abort, strip = true`).

**Risk:** If poker logic + verifier logic exceeds the limit.

**Mitigation:** Split into separate contracts: PokerTable (game logic), ZKVerifier (proof verification), CommitteeRegistry (committee mgmt). Each fits independently.

### A10. LOW: Soroban Storage Costs & TTL

**The Problem:** Soroban uses rent-based storage with TTLs (persistent: 120 days, temporary: 1 day). Active game state needs to survive the duration of play but shouldn't persist forever.

**Mitigation:** Use temporary storage for in-progress games (auto-cleanup). Use persistent storage only for verified results and player balances.

---

## Section B: Improved Plan

### B1. Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        WEB APP (React/Next.js)                  │
│  Wallet (Freighter) │ Game UI │ Card Display │ Betting Controls │
└──────────┬──────────────────────────────┬───────────────────────┘
           │ Soroban SDK                  │ WebSocket/REST
           │ (on-chain txns)              │ (private channels)
           ▼                              ▼
┌─────────────────────┐    ┌──────────────────────────────────────┐
│   SOROBAN CONTRACTS  │    │        MPC COMMITTEE SERVICE         │
│                      │    │                                      │
│  ┌────────────────┐  │    │  ┌─────────┐ ┌─────────┐ ┌────────┐│
│  │  PokerTable    │  │    │  │ Node 0  │ │ Node 1  │ │ Node 2 ││
│  │  (game logic)  │  │    │  │ (coNoir)│ │ (coNoir)│ │(coNoir)││
│  └───────┬────────┘  │    │  └────┬────┘ └────┬────┘ └───┬────┘│
│          │           │    │       └──────┬─────┘──────────┘     │
│  ┌───────▼────────┐  │    │              │                      │
│  │  ZKVerifier    │  │    │  ┌───────────▼──────────────┐       │
│  │  (UltraHonk)   │  │    │  │    Coordinator Service   │       │
│  └────────────────┘  │    │  │  (orchestrates MPC flow) │       │
│                      │    │  └──────────────────────────┘       │
│  ┌────────────────┐  │    └──────────────────────────────────────┘
│  │ CommitteeReg   │  │
│  │ (membership)   │  │
│  └────────────────┘  │
└──────────────────────┘
```

### B2. What is On-Chain vs Off-Chain

**ON-CHAIN (Soroban):**
- Player seat management, buy-in escrow, withdrawals
- Blind structure, turn order enforcement
- Betting actions (check, bet, raise, fold, all-in)
- Timeouts (per-player, per-phase)
- Deck commitment (Poseidon Merkle root)
- Hand commitments (Poseidon hash of each player's hole cards + salt)
- Board card hashes (verified against deck commitment)
- ZK proof verification (deal validity, reveal validity, showdown)
- Pot calculation and settlement
- Committee registration, staking bonds, slashing triggers

**OFF-CHAIN (MPC Committee):**
- Deck shuffle (inside MPC — no proof needed, MPC guarantees correctness)
- Random number generation (commit-reveal from players + MPC entropy)
- Card encryption/delivery (private channel to each player)
- Proof generation (coNoir UltraHonk proofs)
- Private card storage (each node holds a secret share)

### B3. State Machine

```
                    ┌──────────┐
                    │  EMPTY   │ No players seated
                    └────┬─────┘
                         │ join_table(player, buy_in)
                         ▼
                    ┌──────────┐
              ┌────▶│ WAITING  │ 1 player seated, waiting for opponent
              │     └────┬─────┘
              │          │ join_table(player2, buy_in) [min 2 players]
              │          ▼
              │     ┌──────────┐
              │     │ STARTING │ Post blinds, notify committee
              │     └────┬─────┘
              │          │ post_blinds() [auto from contract]
              │          ▼
              │     ┌──────────────┐
              │     │ DEALING      │ Committee shuffles + deals
              │     │              │ → commit deck_root on-chain
              │     │              │ → commit hand_commitments on-chain
              │     │              │ → submit deal_proof on-chain
              │     └────┬─────────┘
              │          │ verify_deal(proof) succeeds
              │          ▼
              │     ┌──────────────┐
              │     │ PREFLOP      │ Betting round
              │     │              │ → player_action(check/bet/raise/fold)
              │     │              │ → timeout: auto-fold after T seconds
              │     └────┬─────────┘
              │          │ betting complete (all called or folded)
              │          │ [if only 1 player remains → SETTLEMENT]
              │          ▼
              │     ┌──────────────┐
              │     │ DEALING_FLOP │ Committee reveals 3 board cards
              │     │              │ → submit reveal_proof(flop_cards)
              │     └────┬─────────┘
              │          │ verify_reveal(proof) succeeds
              │          ▼
              │     ┌──────────────┐
              │     │ FLOP         │ Betting round (same as preflop)
              │     └────┬─────────┘
              │          │
              │          ▼
              │     ┌──────────────┐
              │     │ DEALING_TURN │ Committee reveals 1 card
              │     └────┬─────────┘
              │          │
              │          ▼
              │     ┌──────────────┐
              │     │ TURN         │ Betting round
              │     └────┬─────────┘
              │          │
              │          ▼
              │     ┌───────────────┐
              │     │ DEALING_RIVER │ Committee reveals 1 card
              │     └────┬──────────┘
              │          │
              │          ▼
              │     ┌──────────────┐
              │     │ RIVER        │ Betting round
              │     └────┬─────────┘
              │          │
              │          ▼
              │     ┌──────────────┐
              │     │ SHOWDOWN     │ Remaining players reveal hole cards
              │     │              │ → submit showdown_proof
              │     │              │ → OR players reveal directly
              │     └────┬─────────┘
              │          │
              │          ▼
              │     ┌──────────────┐
              │     │ SETTLEMENT   │ Distribute pot to winner(s)
              │     │              │ → winner can withdraw
              │     └────┬─────────┘
              │          │
              └──────────┘  (next hand if players remain)

TIMEOUT TRANSITIONS (from any betting/dealing phase):
  - Player timeout → auto-fold, continue game
  - Committee timeout → DISPUTE phase
  - DISPUTE → funds returned to players from escrow
```

### B4. Data Formats

#### Card Encoding
```
Card = suit * 13 + rank
  suit: 0=Clubs, 1=Diamonds, 2=Hearts, 3=Spades
  rank: 0=2, 1=3, ..., 8=10, 9=J, 10=Q, 11=K, 12=A

  Examples: 2♣=0, A♣=12, 2♦=13, A♠=51

  Stored as BN254 scalar field element (< 2^254)
```

#### Commitments
```
Card commitment:   Poseidon2(card_value, salt)     → 1 field element
Hand commitment:   Poseidon2(card1_commit, card2_commit) → 1 field element
Deck commitment:   MerkleRoot(Poseidon2, [card_commit_0, ..., card_commit_51])
Board commitment:  Poseidon2(flop1, flop2, flop3, turn, river, board_salt)
```

#### Proof Public Inputs
```
Deal proof:
  - deck_root: Field          (committed deck Merkle root)
  - hand_commitments: [Field; N]  (one per player)
  - card_indices: [u8; N*2]   (which deck positions were dealt)

Reveal proof:
  - deck_root: Field          (same deck commitment)
  - revealed_cards: [Field; K] (plaintext card values for board)
  - card_indices: [u8; K]     (which deck positions)
  - previously_revealed: [u8]  (indices already used — no reuse)

Showdown proof (v1 — public reveal):
  - hand_commitments: [Field; N]  (previously committed)
  - hole_cards: [Field; N*2]      (revealed plaintext)
  - salts: [Field; N*2]           (commitment salts)
  - board_cards: [Field; 5]       (public)
  - winner_index: u8              (declared winner)
  - winner_hand_rank: u32         (hand ranking value)
```

### B5. Circuit Design

#### Circuit A: `deal_valid` (~2K-5K constraints estimated)
Proves: The committed deck was dealt correctly.
```
Private inputs: deck[52], salts[52], player_cards[N][2]
Public inputs:  deck_root, hand_commitments[N], dealt_indices[N*2]

Constraints:
  1. For each card i in deck:
     commit_i = Poseidon2(deck[i], salts[i])
  2. MerkleRoot(commits) == deck_root
  3. For each player p:
     hand_commitment[p] = Poseidon2(
       Poseidon2(deck[dealt_indices[p*2]], salts[dealt_indices[p*2]]),
       Poseidon2(deck[dealt_indices[p*2+1]], salts[dealt_indices[p*2+1]])
     )
  4. All dealt_indices are unique and in range [0, 51]
  5. All deck[i] are in range [0, 51] and unique (valid deck)
```

#### Circuit B: `reveal_board_valid` (~500-1K constraints estimated)
Proves: Revealed community cards match the committed deck.
```
Private inputs: deck[52], salts[52]
Public inputs:  deck_root, revealed_cards[], revealed_indices[],
                previously_dealt_indices[]

Constraints:
  1. For each revealed card:
     commit = Poseidon2(revealed_cards[i], salts[revealed_indices[i]])
     MerkleProof(commit, revealed_indices[i], deck_root) == true
  2. revealed_cards[i] == deck[revealed_indices[i]]
  3. No index in revealed_indices appears in previously_dealt_indices
  4. No duplicate indices
```

#### Circuit C: `showdown_valid` (~5K-15K constraints estimated)
Proves: Given public board + revealed hole cards, the declared winner is correct.
```
Private inputs: (none for v1 — public showdown)
Public inputs:  hole_cards[N][2], board_cards[5],
                hand_commitments[N], salts[N][2],
                winner_index, winner_rank

Constraints:
  1. For each player p:
     verify hand_commitments[p] matches Poseidon2 of their hole cards + salts
  2. For each player p:
     compute hand_rank(hole_cards[p], board_cards) → rank[p]
  3. winner_rank == max(rank[])
  4. winner_index points to player with winner_rank
  5. Hand ranking logic: standard Texas Hold'em (best 5 of 7)
```

### B6. APIs

#### App ↔ Soroban Contracts

```rust
// PokerTable contract
fn create_table(config: TableConfig) -> TableId;
fn join_table(table_id: TableId, buy_in: i128) -> Result<SeatIndex, Error>;
fn leave_table(table_id: TableId) -> Result<i128, Error>;
fn player_action(table_id: TableId, action: Action) -> Result<(), Error>;
fn commit_deal(table_id: TableId, deck_root: BytesN<32>,
               hand_commits: Vec<BytesN<32>>, proof: Bytes) -> Result<(), Error>;
fn commit_reveal(table_id: TableId, cards: Vec<u8>,
                 proof: Bytes) -> Result<(), Error>;
fn submit_showdown(table_id: TableId, hole_cards: Vec<(u8, u8)>,
                   salts: Vec<(BytesN<32>, BytesN<32>)>,
                   proof: Bytes) -> Result<(), Error>;
fn claim_timeout(table_id: TableId) -> Result<(), Error>;
fn withdraw(table_id: TableId) -> Result<i128, Error>;

// Action enum
enum Action { Fold, Check, Call, Bet(i128), Raise(i128), AllIn }
```

#### App ↔ Committee Service (REST/WebSocket)

```
POST /api/table/{id}/request-deal
  → Committee shuffles deck, generates proof, submits to Soroban
  ← { status: "dealing", tx_hash: "..." }

WS /ws/table/{id}/player/{addr}
  → Subscribe to private card delivery
  ← { event: "hole_cards", cards: [card1, card2], salts: [s1, s2] }
  ← { event: "deal_committed", deck_root: "...", tx_hash: "..." }

POST /api/table/{id}/request-reveal/{phase}
  phase: "flop" | "turn" | "river"
  → Committee reveals board cards, generates proof, submits to Soroban
  ← { status: "revealing", cards: [...], tx_hash: "..." }

POST /api/table/{id}/request-showdown
  → Committee generates showdown proof, submits to Soroban
  ← { status: "showdown", winner: "...", tx_hash: "..." }

GET /api/committee/status
  ← { nodes: 3, healthy: 3, epoch: 1, stake: "..." }
```

### B7. Milestone Checklist

- [ ] **M0: Skeleton** — Repo structure, Cargo workspace, basic Soroban contract that compiles
- [ ] **M1: Verifier** — Deploy UltraHonk verifier on Soroban testnet, verify one hardcoded proof, measure CPU cost
- [ ] **M2: Circuits** — Implement Noir circuits (deal_valid, reveal_board_valid, showdown_valid), test locally with `nargo test`
- [ ] **M3: Contract State Machine** — Full poker game logic (no ZK yet): seating, blinds, betting, turns, timeouts, pot settlement
- [ ] **M4: Integration** — Wire circuits to verifier contract; generate proof off-chain, verify on-chain
- [ ] **M5: MPC Service** — Set up coNoir 3-node committee, generate deal proof via MPC, verify on Soroban
- [ ] **M6: Full Hand** — End-to-end: create table → deal → preflop → flop → turn → river → showdown → settlement, all with proofs
- [ ] **M7: Web App** — React UI connecting to Soroban + committee service
- [ ] **M8: Committee Management** — Registry contract, staking placeholders, slashing hooks, rotation
- [ ] **M9: Reusable Library** — Extract stellar-zk-cards package
- [ ] **M10: Polish** — Docker-compose, integration tests, docs

---

## Section C: Repo Structure

```
stellar-poker/
├── Cargo.toml                    # Workspace root
├── README.md                     # Architecture, trust model, how to run
├── docs/
│   ├── protocol-spec.md          # Commitment formats, proof specs, committee model
│   ├── threat-model.md           # Trust assumptions, attack surfaces
│   └── developer-guide.md        # Reusing stellar-zk-cards
│
├── contracts/                    # Soroban smart contracts
│   ├── poker-table/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs            # Contract entry points
│   │       ├── game.rs           # Game state machine
│   │       ├── betting.rs        # Betting logic
│   │       ├── pot.rs            # Pot calculation (side pots)
│   │       ├── timeout.rs        # Timeout enforcement
│   │       ├── types.rs          # Shared types
│   │       └── test.rs           # Unit tests
│   │
│   ├── zk-verifier/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs            # Verify entrypoints
│   │       ├── ultrahonk.rs      # UltraHonk verification wrapper
│   │       └── test.rs
│   │
│   └── committee-registry/
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs            # Committee membership, epoch rotation
│           ├── staking.rs        # Stake/slash placeholders
│           └── test.rs
│
├── circuits/                     # Noir circuits
│   ├── deal_valid/
│   │   ├── Nargo.toml
│   │   └── src/
│   │       └── main.nr           # Deal validity circuit
│   │
│   ├── reveal_board_valid/
│   │   ├── Nargo.toml
│   │   └── src/
│   │       └── main.nr           # Board reveal circuit
│   │
│   ├── showdown_valid/
│   │   ├── Nargo.toml
│   │   └── src/
│   │       └── main.nr           # Showdown circuit
│   │
│   └── lib/                      # Shared Noir library
│       ├── Nargo.toml
│       └── src/
│           ├── lib.nr
│           ├── cards.nr          # Card encoding, hand evaluation
│           ├── commitments.nr    # Poseidon2 commitment helpers
│           └── merkle.nr         # Merkle tree utilities
│
├── stellar-zk-cards/             # Reusable library package
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── encoding.rs           # Card encoding
│       ├── hand_eval.rs          # Hand evaluation (Rust)
│       └── commitment.rs         # Poseidon2 commitment utilities
│
├── services/                     # MPC committee service
│   ├── coordinator/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs           # HTTP/WS server
│   │       ├── api.rs            # REST endpoints
│   │       ├── websocket.rs      # Player card delivery
│   │       ├── mpc.rs            # coNoir MPC orchestration
│   │       ├── deck.rs           # Deck shuffle logic
│   │       └── soroban.rs        # Submit proofs to Soroban
│   │
│   └── node/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs           # MPC node process
│           └── protocol.rs       # REP3 protocol handler
│
├── app/                          # Web frontend
│   ├── package.json
│   ├── next.config.js
│   └── src/
│       ├── app/
│       │   ├── page.tsx          # Lobby
│       │   └── table/[id]/
│       │       └── page.tsx      # Game table
│       ├── components/
│       │   ├── Table.tsx         # Poker table UI
│       │   ├── Cards.tsx         # Card display
│       │   ├── BettingControls.tsx
│       │   ├── PlayerSeat.tsx
│       │   └── PotDisplay.tsx
│       ├── hooks/
│       │   ├── useSoroban.ts     # Soroban contract interaction
│       │   ├── useCommittee.ts   # Committee WebSocket
│       │   └── useGame.ts        # Game state management
│       └── lib/
│           ├── soroban.ts        # Soroban SDK setup
│           └── types.ts          # Shared types
│
├── tests/
│   ├── integration/
│   │   └── full_hand_test.rs     # Full hand e2e test
│   └── property/
│       └── hand_eval_test.rs     # Property tests for hand evaluation
│
├── scripts/
│   ├── deploy.sh                 # Deploy contracts
│   ├── setup-committee.sh        # Initialize committee
│   └── generate-test-proof.sh    # Generate a test proof for verifier testing
│
└── docker-compose.yml            # 3 MPC nodes + coordinator + Soroban localnet
```

---

## Section D: Build Steps (Incremental Phases)

### Phase 1: Foundation (M0)
1. Initialize Cargo workspace with all crates
2. Set up Soroban SDK dependencies
3. Create minimal "hello world" contract that deploys to localnet
4. Set up Noir project structure with nargo
5. Create basic CI (cargo check, nargo check)

### Phase 2: Verifier First (M1) — THE CRITICAL PATH
1. Port/adapt the existing UltraHonk Soroban verifier (indextree/ultrahonk_soroban_contract)
2. Generate a trivial Noir circuit proof locally (e.g., prove knowledge of x where Poseidon2(x) = y)
3. Deploy verifier to Soroban testnet
4. Submit proof on-chain, verify, measure CPU instructions
5. **DECISION GATE:** If verification exceeds 100M instructions, switch to split-verification or Groth16

### Phase 3: Circuits (M2)
1. Implement card encoding + Poseidon2 commitments in Noir library
2. Implement deal_valid circuit with Merkle tree
3. Implement reveal_board_valid circuit
4. Implement hand evaluation in Noir (lookup-table based)
5. Implement showdown_valid circuit
6. Test all circuits with `nargo test` using hardcoded test vectors

### Phase 4: Contract State Machine (M3)
1. Implement PokerTable contract: create_table, join_table, leave_table
2. Implement betting round logic: check, bet, raise, fold, all-in
3. Implement turn order + blind posting
4. Implement timeout mechanism (ledger sequence-based)
5. Implement pot calculation with side pots
6. Implement settlement logic
7. Unit test every state transition

### Phase 5: On-Chain Verification (M4)
1. Wire ZKVerifier contract to PokerTable
2. PokerTable calls ZKVerifier.verify_deal() when committee submits deal proof
3. PokerTable calls ZKVerifier.verify_reveal() for board reveals
4. PokerTable calls ZKVerifier.verify_showdown() for showdown
5. Integration test: generate proof off-chain → submit → verify → state transitions

### Phase 6: MPC Service (M5)
1. Set up coNoir 3-node configuration
2. Implement coordinator that orchestrates deal flow:
   - Receive deal request
   - Coordinate MPC shuffle
   - Generate deal proof via coNoir
   - Submit proof to Soroban
   - Deliver hole cards to players via encrypted channel
3. Test with local 3-node docker setup

### Phase 7: Full Hand (M6)
1. Wire everything together: app → committee → contracts
2. Play one full hand programmatically (scripted test)
3. Verify all proofs on-chain
4. Verify pot settlement

### Phase 8: Web App (M7)
1. Create Next.js app with Freighter wallet integration
2. Implement lobby (create/join tables)
3. Implement game table UI
4. Connect to Soroban for on-chain actions
5. Connect to committee WebSocket for card delivery

### Phase 9: Committee Management (M8)
1. Implement CommitteeRegistry contract
2. Staking bond logic (deposit/withdraw)
3. Slashing trigger events
4. Epoch rotation (simplified: admin-controlled for v1)

### Phase 10: Library + Polish (M9-M10)
1. Extract stellar-zk-cards as standalone package
2. Docker-compose for full local stack
3. Integration test script
4. Documentation

---

## Section E: Implementation

Starting with Phase 1 (Foundation) and Phase 2 (Verifier) — the critical path.
See code files in the repository.
