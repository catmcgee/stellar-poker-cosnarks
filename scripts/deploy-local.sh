#!/usr/bin/env bash
# Deploy contracts and set up on-chain table for local testing.
#
# Prerequisites:
#   - Docker running (for stellar container)
#   - stellar CLI installed
#   - Contracts built: stellar contract build (in each contract dir)
#
# Usage:
#   ./scripts/deploy-local.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

NETWORK="local"
NETWORK_PASSPHRASE="Standalone Network ; February 2017"
RPC_URL="http://localhost:8000/soroban/rpc"
IDENTITY="committee-local"

echo "=== Stellar Poker Local Deployment ==="
echo ""

# 1. Start Stellar standalone container if not already running
echo "Starting Stellar standalone network (Docker)..."
if ! docker ps --format '{{.Names}}' | grep -q stellar; then
    stellar container start -t future --name local --limits unlimited 2>/dev/null || {
        echo "ERROR: Failed to start Stellar container. Is Docker running?"
        exit 1
    }
else
    echo "  Stellar container already running."
fi

# Wait for RPC to be ready
echo "Waiting for RPC to be ready..."
for i in $(seq 1 60); do
    if curl -sf -X POST -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' "$RPC_URL" >/dev/null 2>&1; then
        echo "  RPC ready!"
        break
    fi
    if [ "$i" -eq 60 ]; then
        echo "ERROR: RPC not ready after 120s"
        exit 1
    fi
    sleep 2
done

# 2. Configure network in Stellar CLI
echo "Configuring network..."
stellar network add "$NETWORK" \
    --rpc-url "$RPC_URL" \
    --network-passphrase "$NETWORK_PASSPHRASE" 2>/dev/null || true

# 3. Generate identities and fund them via friendbot
echo "Generating identities..."
FRIENDBOT_URL="http://localhost:8000/friendbot"

for IDENT in "$IDENTITY" player1-local player2-local; do
    stellar keys generate "$IDENT" --overwrite 2>/dev/null || true
    ADDR=$(stellar keys address "$IDENT")
    echo "  Funding $IDENT ($ADDR)..."
    curl -sf "${FRIENDBOT_URL}?addr=${ADDR}" >/dev/null || {
        echo "  WARNING: Friendbot funding failed for $IDENT"
    }
done

COMMITTEE_SECRET=$(stellar keys show "$IDENTITY")
COMMITTEE_ADDRESS=$(stellar keys address "$IDENTITY")
PLAYER1_ADDRESS=$(stellar keys address player1-local)
PLAYER2_ADDRESS=$(stellar keys address player2-local)
echo "  Committee: $COMMITTEE_ADDRESS"
echo "  Player 1:  $PLAYER1_ADDRESS"
echo "  Player 2:  $PLAYER2_ADDRESS"

# 4. Build contracts
echo ""
echo "Building contracts..."
for contract_dir in zk-verifier poker-table committee-registry game-hub; do
    echo "  Building $contract_dir..."
    (cd "$PROJECT_DIR/contracts/$contract_dir" && stellar contract build 2>&1) || {
        echo "ERROR: Failed to build $contract_dir"
        exit 1
    }
done
echo "  All contracts built."

# 5. Deploy contracts
echo ""
echo "Deploying contracts..."

WASM_DIR="$PROJECT_DIR/target/wasm32v1-none/release"

echo "  Deploying zk-verifier..."
ZK_VERIFIER=$(stellar contract deploy \
    --wasm "$WASM_DIR/zk_verifier.wasm" \
    --source "$IDENTITY" \
    --network "$NETWORK")
echo "    ZK Verifier: $ZK_VERIFIER"

echo "  Deploying game-hub..."
GAME_HUB=$(stellar contract deploy \
    --wasm "$WASM_DIR/game_hub.wasm" \
    --source "$IDENTITY" \
    --network "$NETWORK")
echo "    Game Hub: $GAME_HUB"

echo "  Deploying poker-table..."
POKER_TABLE=$(stellar contract deploy \
    --wasm "$WASM_DIR/poker_table.wasm" \
    --source "$IDENTITY" \
    --network "$NETWORK")
echo "    Poker Table: $POKER_TABLE"

echo "  Deploying committee-registry..."
COMMITTEE_REGISTRY=$(stellar contract deploy \
    --wasm "$WASM_DIR/committee_registry.wasm" \
    --source "$IDENTITY" \
    --network "$NETWORK")
echo "    Committee Registry: $COMMITTEE_REGISTRY"

# 6. Deploy SAC (Stellar Asset Contract) for native XLM as token
echo ""
echo "Deploying native XLM SAC token..."
TOKEN_CONTRACT=$(stellar contract asset deploy \
    --asset native \
    --source "$IDENTITY" \
    --network "$NETWORK" 2>/dev/null) || {
    # SAC may already be deployed
    TOKEN_CONTRACT=$(stellar contract asset id \
        --asset native \
        --network "$NETWORK" 2>/dev/null) || TOKEN_CONTRACT=""
}
echo "  Token (native XLM SAC): $TOKEN_CONTRACT"

# 7. Initialize zk-verifier
echo ""
echo "Initializing zk-verifier..."
stellar contract invoke \
    --id "$ZK_VERIFIER" \
    --source "$IDENTITY" \
    --network "$NETWORK" \
    -- initialize \
    --admin "$COMMITTEE_ADDRESS" || echo "  (may already be initialized)"

# 8. Upload verification keys (convert from BB format first)
echo ""
echo "Uploading verification keys..."
for circuit in deal_valid reveal_board_valid showdown_valid; do
    VK_PATH="$PROJECT_DIR/circuits/$circuit/target/vk"
    VK_COMPACT="$PROJECT_DIR/circuits/$circuit/target/vk.compact"
    VK_KECCAK="$PROJECT_DIR/circuits/$circuit/target/vk_keccak"
    if [ -f "$VK_PATH" ]; then
        # Convert BB VK (3680 bytes, limb-encoded) to compact + keccak formats
        echo "  Converting VK for $circuit..."
        python3 "$PROJECT_DIR/scripts/convert-vk.py" "$VK_PATH" "$VK_COMPACT" "$VK_KECCAK" || {
            echo "    WARNING: VK conversion failed for $circuit"
            continue
        }
        VK_HEX=$(xxd -p "$VK_COMPACT" | tr -d '\n')
        case "$circuit" in
            deal_valid)         CIRCUIT_TYPE="DealValid" ;;
            reveal_board_valid) CIRCUIT_TYPE="RevealBoardValid" ;;
            showdown_valid)     CIRCUIT_TYPE="ShowdownValid" ;;
        esac
        echo "  Uploading VK for $circuit ($CIRCUIT_TYPE)..."
        stellar contract invoke \
            --id "$ZK_VERIFIER" \
            --source "$IDENTITY" \
            --network "$NETWORK" \
            -- set_verification_key \
            --admin "$COMMITTEE_ADDRESS" \
            --circuit '"'"$CIRCUIT_TYPE"'"' \
            --vk_data "$VK_HEX" || echo "    WARNING: VK upload failed for $circuit"
    else
        echo "  SKIP: No VK file at $VK_PATH (compile circuits first)"
    fi
done

# 9. Create poker table on-chain
echo ""
echo "Creating poker table on-chain..."
TABLE_ID=$(stellar contract invoke \
    --id "$POKER_TABLE" \
    --source "$IDENTITY" \
    --network "$NETWORK" \
    -- create_table \
    --admin "$COMMITTEE_ADDRESS" \
    --config "{\"token\":\"$TOKEN_CONTRACT\",\"min_buy_in\":\"1000000000\",\"max_buy_in\":\"100000000000\",\"small_blind\":\"500000000\",\"big_blind\":\"1000000000\",\"max_players\":2,\"timeout_ledgers\":100,\"committee\":\"$COMMITTEE_ADDRESS\",\"verifier\":\"$ZK_VERIFIER\",\"game_hub\":\"$GAME_HUB\"}")
echo "  Table ID: $TABLE_ID"

# 10. Mint/wrap XLM for players and have them join
echo ""
echo "Setting up players..."

# Players need to approve token transfers - first wrap some XLM for them
# On local standalone, funded accounts have 10000 XLM = 100000000000 stroops
BUY_IN=10000000000  # 1000 XLM in stroops

echo "  Player 1 joining table..."
stellar contract invoke \
    --id "$POKER_TABLE" \
    --source player1-local \
    --network "$NETWORK" \
    -- join_table \
    --table_id "$TABLE_ID" \
    --player "$PLAYER1_ADDRESS" \
    --buy_in "$BUY_IN" || echo "  WARNING: Player 1 join failed"

echo "  Player 2 joining table..."
stellar contract invoke \
    --id "$POKER_TABLE" \
    --source player2-local \
    --network "$NETWORK" \
    -- join_table \
    --table_id "$TABLE_ID" \
    --player "$PLAYER2_ADDRESS" \
    --buy_in "$BUY_IN" || echo "  WARNING: Player 2 join failed"

# 11. Start hand (sets phase to Dealing)
echo ""
echo "Starting hand..."
stellar contract invoke \
    --id "$POKER_TABLE" \
    --source "$IDENTITY" \
    --network "$NETWORK" \
    -- start_hand \
    --table_id "$TABLE_ID" || echo "  WARNING: start_hand failed"

# 12. Write environment file
ENV_FILE="$PROJECT_DIR/.env.local"
cat > "$ENV_FILE" << EOF
# Generated by deploy-local.sh — $(date -u +"%Y-%m-%dT%H:%M:%SZ")
SOROBAN_RPC=$RPC_URL
POKER_TABLE_CONTRACT=$POKER_TABLE
ZK_VERIFIER_CONTRACT=$ZK_VERIFIER
COMMITTEE_REGISTRY_CONTRACT=$COMMITTEE_REGISTRY
GAME_HUB_CONTRACT=$GAME_HUB
TOKEN_CONTRACT=$TOKEN_CONTRACT
TABLE_ID=$TABLE_ID
COMMITTEE_SECRET=$COMMITTEE_SECRET
COMMITTEE_ADDRESS=$COMMITTEE_ADDRESS
PLAYER1_ADDRESS=$PLAYER1_ADDRESS
PLAYER2_ADDRESS=$PLAYER2_ADDRESS
NETWORK_PASSPHRASE="$NETWORK_PASSPHRASE"
EOF

echo ""
echo "=== Deployment Complete ==="
echo ""
echo "  Poker Table:        $POKER_TABLE"
echo "  ZK Verifier:        $ZK_VERIFIER"
echo "  Game Hub:           $GAME_HUB"
echo "  Committee Registry: $COMMITTEE_REGISTRY"
echo "  Token (native SAC): $TOKEN_CONTRACT"
echo "  On-chain Table ID:  $TABLE_ID"
echo "  Committee Address:  $COMMITTEE_ADDRESS"
echo "  Player 1:           $PLAYER1_ADDRESS"
echo "  Player 2:           $PLAYER2_ADDRESS"
echo ""
echo "  Environment written to: $ENV_FILE"
echo ""
echo "  Next: run ./scripts/start-local.sh to start services"
