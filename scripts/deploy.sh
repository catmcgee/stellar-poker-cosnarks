#!/usr/bin/env bash
set -euo pipefail

# Stellar Poker - Deploy Script
# Deploys contracts to Soroban testnet and starts services

NETWORK="${NETWORK:-testnet}"
SOROBAN_RPC="${SOROBAN_RPC:-https://soroban-testnet.stellar.org}"
SOROBAN_NETWORK_PASSPHRASE="${SOROBAN_NETWORK_PASSPHRASE:-Test SDF Network ; September 2015}"

echo "=== Stellar Poker Deploy ==="
echo "Network: $NETWORK"
echo "RPC: $SOROBAN_RPC"
echo ""

# Check dependencies
command -v stellar >/dev/null 2>&1 || { echo "stellar CLI not found. Install: cargo install stellar-cli"; exit 1; }
command -v nargo >/dev/null 2>&1 || { echo "nargo not found. Install: noirup -v 1.0.0-beta.13"; exit 1; }

# --- Step 1: Build Soroban contracts ---
echo "=== Building Soroban contracts ==="
cargo build --release --target wasm32-unknown-unknown \
  -p poker-table \
  -p zk-verifier \
  -p committee-registry

echo "Optimizing WASM..."
for contract in poker_table zk_verifier committee_registry; do
  stellar contract optimize \
    --wasm "target/wasm32-unknown-unknown/release/${contract}.wasm" 2>/dev/null || true
done

# --- Step 2: Compile Noir circuits ---
echo ""
echo "=== Compiling Noir circuits ==="
for circuit in deal_valid reveal_board_valid showdown_valid; do
  echo "  Compiling $circuit..."
  (cd "circuits/$circuit" && nargo compile)
done

# --- Step 3: Generate deployer identity ---
echo ""
echo "=== Setting up deployer identity ==="
if ! stellar keys show deployer >/dev/null 2>&1; then
  stellar keys generate deployer --network "$NETWORK"
  echo "Funding deployer account..."
  stellar keys fund deployer --network "$NETWORK" || true
fi

DEPLOYER=$(stellar keys address deployer)
echo "Deployer: $DEPLOYER"

# --- Step 4: Deploy contracts ---
echo ""
echo "=== Deploying contracts ==="

echo "Deploying zk-verifier..."
ZK_VERIFIER_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/zk_verifier.wasm \
  --source deployer \
  --network "$NETWORK" 2>/dev/null)
echo "  ZK Verifier: $ZK_VERIFIER_ID"

echo "Deploying committee-registry..."
COMMITTEE_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/committee_registry.wasm \
  --source deployer \
  --network "$NETWORK" 2>/dev/null)
echo "  Committee Registry: $COMMITTEE_ID"

echo "Deploying poker-table..."
POKER_TABLE_ID=$(stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/poker_table.wasm \
  --source deployer \
  --network "$NETWORK" 2>/dev/null)
echo "  Poker Table: $POKER_TABLE_ID"

# --- Step 5: Initialize contracts ---
echo ""
echo "=== Initializing contracts ==="

stellar contract invoke \
  --id "$ZK_VERIFIER_ID" \
  --source deployer \
  --network "$NETWORK" \
  -- initialize --admin "$DEPLOYER" 2>/dev/null

stellar contract invoke \
  --id "$COMMITTEE_ID" \
  --source deployer \
  --network "$NETWORK" \
  -- initialize --admin "$DEPLOYER" 2>/dev/null

echo ""
echo "=== Deploy Complete ==="
echo ""
echo "Contract Addresses:"
echo "  ZK_VERIFIER=$ZK_VERIFIER_ID"
echo "  COMMITTEE_REGISTRY=$COMMITTEE_ID"
echo "  POKER_TABLE=$POKER_TABLE_ID"
echo ""
echo "Next steps:"
echo "  1. Set verification keys: stellar contract invoke --id $ZK_VERIFIER_ID -- set_verification_key ..."
echo "  2. Register committee members: stellar contract invoke --id $COMMITTEE_ID -- register_member ..."
echo "  3. Start MPC nodes: docker-compose up mpc-node-0 mpc-node-1 mpc-node-2"
echo "  4. Start coordinator: docker-compose up coordinator"
echo "  5. Start web app: cd app && npm run dev"
