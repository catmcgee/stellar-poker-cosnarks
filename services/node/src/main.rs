//! Stellar Poker MPC Node
//!
//! Each node is a participant in the REP3 MPC protocol via TACEO's co-noir.
//! It holds secret shares and participates in collaborative proof generation.
//!
//! Lifecycle:
//! 1. Coordinator POSTs shares to /session/:id/shares
//! 2. Coordinator triggers proof gen via POST /session/:id/generate
//! 3. Node runs co-noir generate-witness + build-and-generate-proof as subprocesses
//! 4. Coordinator polls GET /session/:id/status and retrieves proof via GET /session/:id/proof
//!
//! co-noir handles peer-to-peer MPC communication internally via TCP (ports 10000-10002).

use axum::{
    routing::{get, post},
    Router,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

mod api;
mod session;

use session::MpcSessionState;

#[derive(Clone)]
pub struct NodeState {
    pub node_id: u32,
    pub sessions: Arc<RwLock<HashMap<String, Arc<RwLock<MpcSessionState>>>>>,
    pub party_config_path: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let node_id: u32 = std::env::var("NODE_ID")
        .unwrap_or_else(|_| "0".to_string())
        .parse()
        .unwrap();
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| format!("{}", 8101 + node_id))
        .parse()
        .unwrap();
    let party_config_path = std::env::var("PARTY_CONFIG")
        .unwrap_or_else(|_| format!("./config/party_{}.toml", node_id));

    tracing::info!("MPC Node {} starting on port {}", node_id, port);
    tracing::info!("Party config: {}", party_config_path);

    let state = NodeState {
        node_id,
        sessions: Arc::new(RwLock::new(HashMap::new())),
        party_config_path,
    };

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/session/{id}/shares", post(api::post_shares))
        .route("/session/{id}/generate", post(api::post_generate))
        .route("/session/{id}/status", get(api::get_status))
        .route("/session/{id}/proof", get(api::get_proof))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
