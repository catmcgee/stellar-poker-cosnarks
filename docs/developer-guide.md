# Stellar Poker Developer Guide

## Local Development Setup

### Prerequisites

1. **Rust** (latest stable): https://rustup.rs
   ```bash
   rustup target add wasm32-unknown-unknown
   ```

2. **Nargo** (Noir compiler) v1.0.0-beta.17:
   ```bash
   curl -L https://raw.githubusercontent.com/noir-lang/noirup/refs/heads/main/install | bash
   noirup -v 1.0.0-beta.17
   ```

3. **Stellar CLI**:
   ```bash
   cargo install stellar-cli --features opt
   ```

4. **Node.js** 18+ and npm

5. **Docker** (for local Soroban network)

### Quick Setup

```bash
./scripts/setup.sh
```

This checks all dependencies, builds Rust crates, compiles Noir circuits, runs tests, and builds the web app.

## Project Layout

### Soroban Contracts (`contracts/`)

Each contract is a separate Rust crate targeting `wasm32-unknown-unknown`:

- **poker-table**: Main game logic. State machine for Texas Hold'em with on-chain betting, pot management, and settlement.
- **zk-verifier**: Verifies UltraHonk proofs using BN254 native host functions. Currently a placeholder (accepts all proofs); will be integrated with the real verifier.
- **committee-registry**: Manages MPC committee membership, staking, and slashing.

Build contracts:
```bash
cargo build --release --target wasm32-unknown-unknown -p poker-table
```

### Noir Circuits (`circuits/`)

Each circuit is a Nargo package:

- **lib**: Shared library with card encoding, Poseidon2 commitments, Merkle trees, hand evaluation
- **deal_valid**: Proves a deal is consistent with the committed deck
- **reveal_board_valid**: Proves revealed community cards match the deck
- **showdown_valid**: Proves the declared winner has the best hand

Compile and test:
```bash
./scripts/compile-circuits.sh
cd circuits/lib && nargo test
```

Key Noir patterns used:
- **No early returns**: Noir doesn't support `return` mid-function. Use conditional accumulation instead.
- **Poseidon2**: Use `std::hash::poseidon2_permutation([a, b, 0, 0], 4)[0]`. State size is always 4 for BN254.
- **ASCII only**: Noir comments don't support non-ASCII characters.

### MPC Services (`services/`)

- **coordinator**: Axum HTTP server (port 8080). Orchestrates MPC sessions, routes messages between MPC nodes and the blockchain.
- **node**: Minimal MPC node. In production, runs TACEO coNoir runtime.

### Web App (`app/`)

Next.js with TypeScript and Tailwind CSS:
- `/` - Lobby page (table selection)
- `/table/[id]` - Game table with card rendering, betting UI, board display

## Adding a New Circuit

1. Create directory: `circuits/my_circuit/`
2. Create `Nargo.toml`:
   ```toml
   [package]
   name = "my_circuit"
   type = "bin"
   compiler_version = ">=0.36.0"

   [dependencies]
   stellar_poker_lib = { path = "../lib" }
   ```
3. Create `src/main.nr` with your circuit logic
4. Compile: `cd circuits/my_circuit && nargo compile`

## Adding a New Contract Entry Point

1. Add the function to `contracts/poker-table/src/lib.rs` inside `#[contractimpl]`
2. Add required types to `src/types.rs`
3. Implement logic in the appropriate module (`game.rs`, `betting.rs`, etc.)
4. Run `cargo check -p poker-table`

## Testing

### Noir circuit tests
```bash
cd circuits/lib && nargo test
```

### Rust contract tests
```bash
cargo test -p poker-table
```

### Coordinator unit tests
```bash
cargo test -p coordinator
```

## API Reference

### Coordinator REST API

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/health` | Health check |
| POST | `/api/table/{id}/request-deal` | Trigger MPC shuffle + deal |
| POST | `/api/table/{id}/request-reveal/{phase}` | Reveal flop/turn/river |
| POST | `/api/table/{id}/request-showdown` | Trigger showdown |
| GET | `/api/table/{id}/player/{addr}/cards` | Get player's private cards |
| GET | `/api/committee/status` | MPC committee health |

### Soroban Contract Functions

**PokerTable**:
- `create_table(admin, config) -> u32`
- `join_table(table_id, player, buy_in) -> u32`
- `leave_table(table_id, player) -> i128`
- `start_hand(table_id)`
- `commit_deal(table_id, committee, deck_root, hand_commitments, dealt_indices, proof, public_inputs)`
- `player_action(table_id, player, action)`
- `reveal_board(table_id, committee, cards, indices, proof, public_inputs)`
- `submit_showdown(table_id, committee, hole_cards, salts, proof, public_inputs)`
- `claim_timeout(table_id, claimer)`
- `get_table(table_id) -> TableState`

## Common Issues

**"Requirements may only refer to full releases"**: Use `compiler_version = ">=0.36.0"` instead of beta version strings in Nargo.toml.

**"poseidon2 is private"**: Use `std::hash::poseidon2_permutation()` not `std::hash::poseidon2::Poseidon2::hash()`.

**"Expected 4 values but encountered N"**: Poseidon2 state size is always 4 for BN254. Use `poseidon2_permutation([a, b, 0, 0], 4)`.

**"Early 'return' is unsupported"**: Refactor to use conditional accumulation with a flag variable instead of early returns.
