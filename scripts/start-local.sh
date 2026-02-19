#!/usr/bin/env bash
# Start all services locally for development/testing.
#
# Prerequisites:
#   - co-noir installed: cargo install --git https://github.com/TaceoLabs/co-snarks --branch main co-noir
#   - CRS downloaded: ./scripts/download-crs.sh
#   - Circuits compiled: (cd circuits/deal_valid && nargo compile) etc.
#
# Usage:
#   ./scripts/start-local.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
CONFIG_DIR="${PROJECT_DIR}/services/node/config/local"

echo "=== Starting Stellar Poker MPC services locally ==="
echo ""

# Check prerequisites
command -v co-noir >/dev/null 2>&1 || { echo "ERROR: co-noir not found. Install with: cargo install --git https://github.com/TaceoLabs/co-snarks --branch main co-noir"; exit 1; }

for circuit in deal_valid reveal_board_valid showdown_valid; do
    if [ ! -f "${PROJECT_DIR}/circuits/${circuit}/target/${circuit}.json" ]; then
        echo "ERROR: Circuit ${circuit} not compiled. Run: (cd circuits/${circuit} && nargo compile)"
        exit 1
    fi
done

echo "Starting MPC Node 0 (port 8101)..."
NODE_ID=0 PORT=8101 PARTY_CONFIG="${CONFIG_DIR}/party_0.toml" \
    cargo run -p mpc-node --quiet &
PID_NODE0=$!

echo "Starting MPC Node 1 (port 8102)..."
NODE_ID=1 PORT=8102 PARTY_CONFIG="${CONFIG_DIR}/party_1.toml" \
    cargo run -p mpc-node --quiet &
PID_NODE1=$!

echo "Starting MPC Node 2 (port 8103)..."
NODE_ID=2 PORT=8103 PARTY_CONFIG="${CONFIG_DIR}/party_2.toml" \
    cargo run -p mpc-node --quiet &
PID_NODE2=$!

sleep 2

echo "Starting Coordinator (port 8080)..."
CIRCUIT_DIR="${PROJECT_DIR}/circuits" \
CRS_DIR="${PROJECT_DIR}/crs" \
BIND_ADDR="0.0.0.0:8080" \
    cargo run -p coordinator --quiet &
PID_COORD=$!

sleep 1

echo ""
echo "=== All services started ==="
echo "  Node 0: http://localhost:8101  (PID: ${PID_NODE0})"
echo "  Node 1: http://localhost:8102  (PID: ${PID_NODE1})"
echo "  Node 2: http://localhost:8103  (PID: ${PID_NODE2})"
echo "  Coordinator: http://localhost:8080  (PID: ${PID_COORD})"
echo ""
echo "Test with:"
echo "  curl -s http://localhost:8080/api/health"
echo "  curl -s -X POST http://localhost:8080/api/table/1/request-deal"
echo ""
echo "Press Ctrl+C to stop all services"

cleanup() {
    echo ""
    echo "Stopping services..."
    kill $PID_NODE0 $PID_NODE1 $PID_NODE2 $PID_COORD 2>/dev/null || true
    wait 2>/dev/null
    echo "Done."
}

trap cleanup EXIT INT TERM
wait
