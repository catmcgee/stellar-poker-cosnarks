//! Stellar Poker MPC Coordinator Service
//!
//! This service orchestrates the MPC committee for:
//! 1. Deck shuffling (using TACEO coNoir's REP3 MPC)
//! 2. Proof generation (deal, reveal, showdown proofs via coNoir)
//! 3. Private card delivery to players
//! 4. Submitting proofs to Soroban
//!
//! Architecture:
//! - The coordinator receives requests from the web app
//! - It orchestrates 3 MPC nodes running coNoir
//! - Each node holds a secret share of the deck
//! - No single node (or the coordinator) knows the full deck
//! - Proofs are generated collaboratively and are identical to standard
//!   Barretenberg/UltraHonk proofs

use axum::{
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;

mod api;
mod crypto;
mod deck;
mod hand_eval;
mod mpc;
mod soroban;

#[derive(Clone)]
struct AppState {
    tables: Arc<RwLock<HashMap<u32, TableSession>>>,
    mpc_config: MpcConfig,
    soroban_config: soroban::SorobanConfig,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TableSession {
    table_id: u32,
    /// The shuffled deck (only assembled during MPC, then split into shares)
    /// In production, this is NEVER stored in plaintext on the coordinator.
    /// It exists only as secret shares across the 3 MPC nodes.
    deck_order: Option<Vec<u32>>,
    /// Per-card salts for commitments
    card_salts: Option<Vec<String>>,
    /// Deck Merkle root (public, posted on-chain)
    deck_root: Option<String>,
    /// Player hole card assignments (indices into deck)
    player_cards: HashMap<String, (u32, u32)>,
    /// Cards already dealt (indices)
    dealt_indices: Vec<u32>,
    /// Board card indices
    board_indices: Vec<u32>,
    /// Current game phase
    phase: String,
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
        circuit_dir: std::env::var("CIRCUIT_DIR")
            .unwrap_or_else(|_| "./circuits".to_string()),
        soroban_rpc: std::env::var("SOROBAN_RPC")
            .unwrap_or_else(|_| "http://localhost:8000/soroban/rpc".to_string()),
        committee_secret: std::env::var("COMMITTEE_SECRET")
            .unwrap_or_else(|_| "test_secret".to_string()),
    };

    let soroban_config = soroban::SorobanConfig::from_env();
    if soroban_config.is_configured() {
        tracing::info!("Soroban configured: contract={}", soroban_config.poker_table_contract);
    } else {
        tracing::warn!("Soroban not configured — on-chain submission disabled");
    }

    let state = AppState {
        tables: Arc::new(RwLock::new(HashMap::new())),
        mpc_config,
        soroban_config,
    };

    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/table/{table_id}/request-deal", post(api::request_deal))
        .route(
            "/api/table/{table_id}/request-reveal/{phase}",
            post(api::request_reveal),
        )
        .route(
            "/api/table/{table_id}/request-showdown",
            post(api::request_showdown),
        )
        .route(
            "/api/table/{table_id}/player/{address}/cards",
            get(api::get_player_cards),
        )
        .route(
            "/api/table/{table_id}/state",
            get(api::get_table_state),
        )
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
