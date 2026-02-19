#!/usr/bin/env bash
set -euo pipefail

# Stellar Poker - Development Setup
# Installs all dependencies and verifies the build

echo "=== Stellar Poker Development Setup ==="
echo ""

# --- Check Rust ---
echo "Checking Rust..."
if ! command -v rustc >/dev/null 2>&1; then
  echo "  Rust not found. Install from https://rustup.rs"
  exit 1
fi
echo "  Rust $(rustc --version | cut -d' ' -f2)"

# Add wasm target
echo "  Adding wasm32-unknown-unknown target..."
rustup target add wasm32-unknown-unknown 2>/dev/null || true

# --- Check Stellar CLI ---
echo "Checking Stellar CLI..."
if ! command -v stellar >/dev/null 2>&1; then
  echo "  Installing stellar-cli..."
  cargo install stellar-cli --features opt
else
  echo "  Stellar CLI $(stellar --version 2>/dev/null || echo 'installed')"
fi

# --- Check Nargo ---
echo "Checking Nargo (Noir)..."
if ! command -v nargo >/dev/null 2>&1; then
  echo "  Nargo not found. Install with:"
  echo "    curl -L https://raw.githubusercontent.com/noir-lang/noirup/refs/heads/main/install | bash"
  echo "    noirup -v 1.0.0-beta.13"
  exit 1
fi
echo "  $(nargo --version)"

# --- Check Node.js ---
echo "Checking Node.js..."
if ! command -v node >/dev/null 2>&1; then
  echo "  Node.js not found. Install from https://nodejs.org"
  exit 1
fi
echo "  Node.js $(node --version)"

# --- Build Rust workspace ---
echo ""
echo "=== Building Rust workspace ==="
cargo check
echo "  All crates compile."

# --- Check Noir circuits ---
echo ""
echo "=== Checking Noir circuits ==="
for circuit in lib deal_valid reveal_board_valid showdown_valid; do
  echo "  Checking $circuit..."
  (cd "circuits/$circuit" && nargo check 2>/dev/null)
done

# --- Run Noir tests ---
echo ""
echo "=== Running Noir tests ==="
(cd circuits/lib && nargo test)

# --- Install web app dependencies ---
echo ""
echo "=== Installing web app dependencies ==="
(cd app && npm install)

# --- Build web app ---
echo ""
echo "=== Building web app ==="
(cd app && npx next build)

echo ""
echo "=== Setup Complete ==="
echo ""
echo "To run in development mode:"
echo "  1. Start local Soroban:    docker-compose up soroban"
echo "  2. Start MPC nodes:        docker-compose up mpc-node-0 mpc-node-1 mpc-node-2"
echo "  3. Start coordinator:      docker run the coordinator or: cargo run -p coordinator"
echo "  4. Start web app:          cd app && npm run dev"
echo ""
echo "Or start everything:         docker-compose up"
