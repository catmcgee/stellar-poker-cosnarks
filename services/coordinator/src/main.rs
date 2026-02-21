//! Stellar Poker MPC Coordinator Service
//!
//! This service orchestrates the MPC committee for:
//! 1. Distributed share preparation across all MPC nodes (coNoir split-input)
//! 2. Proof generation (deal, reveal, showdown proofs via coNoir)
//! 3. Submitting proofs to Soroban
//!
//! Architecture:
//! - The coordinator receives requests from the web app
//! - It orchestrates 3 MPC nodes running coNoir
//! - Each node prepares only its own private witness contribution
//! - Coordinator never sees plaintext deck/salts/hole cards
//! - Proofs are generated collaboratively and are identical to standard
//!   Barretenberg/UltraHonk proofs

use axum::{
    routing::{get, post},
    Router,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

mod api;
mod mpc;
mod soroban;

#[derive(Clone)]
struct AppState {
    tables: Arc<RwLock<HashMap<u32, TableSession>>>,
    lobby_assignments: Arc<RwLock<HashMap<u32, HashMap<String, String>>>>,
    mpc_config: MpcConfig,
    soroban_config: soroban::SorobanConfig,
    auth_state: Arc<RwLock<AuthState>>,
    rate_limit_state: Arc<RwLock<RateLimitState>>,
}

#[derive(Clone)]
#[allow(dead_code)]
struct MpcConfig {
    /// Endpoints of the 3 MPC nodes
    node_endpoints: Vec<String>,
    /// Path to compiled Noir circuits (ACIR)
    circuit_dir: String,
    /// Soroban RPC endpoint
    soroban_rpc: String,
    /// Committee signing key (for submitting txns)
    committee_secret: String,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct TableSession {
    table_id: u32,
    /// Deck Merkle root (public, posted on-chain)
    deck_root: String,
    /// Per-player hand commitments in seat order.
    hand_commitments: Vec<String>,
    /// Players in deterministic seat order.
    player_order: Vec<String>,
    /// Cards already dealt/revealed (indices).
    dealt_indices: Vec<u32>,
    /// Per-player dealt card positions: (card1_deck_pos, card2_deck_pos).
    player_card_positions: Vec<(u32, u32)>,
    /// Revealed board indices.
    board_indices: Vec<u32>,
    /// Current game phase.
    phase: String,
    /// Last deal proof session ID.
    deal_session_id: String,
    /// Latest deal tx hash, if submitted.
    deal_tx_hash: Option<String>,
    /// Reveal tx hashes by phase.
    reveal_tx_hashes: HashMap<String, String>,
    /// Reveal proof session IDs by phase.
    reveal_session_ids: HashMap<String, String>,
    /// Revealed cards by phase.
    revealed_cards_by_phase: HashMap<String, Vec<u32>>,
    /// Latest showdown tx hash, if submitted.
    showdown_tx_hash: Option<String>,
    /// Last showdown proof session ID, if submitted.
    showdown_session_id: Option<String>,
    /// Cached showdown result for idempotent retries.
    showdown_result: Option<(String, u32)>,
    /// Monotonic nonce for unique proof session IDs.
    proof_nonce: u64,
}

#[derive(Clone, Debug, Default)]
struct AuthState {
    last_nonce_by_address: HashMap<String, u64>,
}

#[derive(Clone, Debug, Default)]
struct RateLimitState {
    requests_by_bucket: HashMap<String, Vec<u64>>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let mpc_config = MpcConfig {
        node_endpoints: vec![
            std::env::var("MPC_NODE_0").unwrap_or_else(|_| "http://localhost:8101".to_string()),
            std::env::var("MPC_NODE_1").unwrap_or_else(|_| "http://localhost:8102".to_string()),
            std::env::var("MPC_NODE_2").unwrap_or_else(|_| "http://localhost:8103".to_string()),
        ],
        circuit_dir: std::env::var("CIRCUIT_DIR").unwrap_or_else(|_| "./circuits".to_string()),
        soroban_rpc: std::env::var("SOROBAN_RPC")
            .unwrap_or_else(|_| "http://localhost:8000/soroban/rpc".to_string()),
        committee_secret: std::env::var("COMMITTEE_SECRET")
            .unwrap_or_else(|_| "test_secret".to_string()),
    };

    let soroban_config = soroban::SorobanConfig::from_env();
    if soroban_config.is_configured() {
        tracing::info!(
            "Soroban configured: contract={}",
            soroban_config.poker_table_contract
        );
    } else {
        tracing::warn!("Soroban not configured — on-chain submission disabled");
    }

    let state = AppState {
        tables: Arc::new(RwLock::new(HashMap::new())),
        lobby_assignments: Arc::new(RwLock::new(HashMap::new())),
        mpc_config,
        soroban_config,
        auth_state: Arc::new(RwLock::new(AuthState::default())),
        rate_limit_state: Arc::new(RwLock::new(RateLimitState::default())),
    };

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/tables/create", post(api::create_table))
        .route("/api/tables/open", get(api::list_open_tables))
        .route("/api/table/:table_id/join", post(api::join_table))
        .route("/api/table/:table_id/lobby", get(api::get_table_lobby))
        .route(
            "/api/table/:table_id/request-deal",
            post(api::request_deal),
        )
        .route(
            "/api/table/:table_id/request-reveal/:phase",
            post(api::request_reveal),
        )
        .route(
            "/api/table/:table_id/request-showdown",
            post(api::request_showdown),
        )
        .route(
            "/api/table/:table_id/player/:address/cards",
            get(api::get_player_cards),
        )
        .route("/api/table/:table_id/state", get(api::get_table_state))
        .route("/api/committee/status", get(api::committee_status))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    tracing::info!("Coordinator listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> &'static str {
    "ok"
}
