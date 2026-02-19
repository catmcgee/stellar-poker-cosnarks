//! HTTP API handlers for the MPC node.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::session::{self, MpcSessionState, SessionStatus};
use crate::NodeState;

#[derive(Deserialize)]
pub struct SharesRequest {
    pub circuit_name: String,
    pub share_data: String, // base64-encoded share file
}

#[derive(Serialize)]
pub struct StatusResponse {
    pub session_id: String,
    pub status: String,
}

#[derive(Deserialize)]
pub struct GenerateRequest {
    pub circuit_dir: String,
    pub crs_path: String,
}

/// POST /session/:id/shares
///
/// Receive secret-shared input from the coordinator.
pub async fn post_shares(
    State(state): State<NodeState>,
    Path(session_id): Path<String>,
    Json(req): Json<SharesRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let work_dir = tempfile::tempdir()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("tmpdir: {}", e)))?;

    let work_path = work_dir.keep();

    let mut session = MpcSessionState::new(
        session_id.clone(),
        req.circuit_name.clone(),
        work_path,
    );

    session::receive_shares(&mut session, &req.share_data)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;

    state.sessions.write().await.insert(session_id, Arc::new(RwLock::new(session)));

    Ok(StatusCode::OK)
}

/// POST /session/:id/generate
///
/// Trigger MPC proof generation in the background.
pub async fn post_generate(
    State(state): State<NodeState>,
    Path(session_id): Path<String>,
    Json(req): Json<GenerateRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    let sessions = state.sessions.read().await;
    let session_lock = sessions
        .get(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "session not found".to_string()))?
        .clone();

    let mut session = session_lock.write().await;
    session.status = SessionStatus::WitnessGenerating;

    let sid = session_id.clone();
    let circuit_dir = req.circuit_dir.clone();
    let circuit_name = session.circuit_name.clone();
    let work_dir = session.work_dir.clone();
    let node_id = state.node_id;
    let party_config = state.party_config_path.clone();
    let crs_path = req.crs_path.clone();

    let session_lock_bg = session_lock.clone();
    drop(session); // release write lock before spawning

    // Spawn proof generation in background
    tokio::spawn(async move {
        let result = session::run_proof_generation(
            sid.clone(),
            circuit_dir,
            circuit_name,
            work_dir.clone(),
            node_id,
            party_config,
            crs_path,
        )
        .await;

        let mut session = session_lock_bg.write().await;
        match result {
            Ok(proof_bytes) => {
                let proof_path = work_dir.join("proof.bin");
                // proof bytes already written by co-noir, but ensure we track it
                if let Err(e) = std::fs::write(&proof_path, &proof_bytes) {
                    session.status = SessionStatus::Failed(format!("write proof: {}", e));
                    return;
                }
                session.proof_path = Some(proof_path);
                session.status = SessionStatus::Complete;
                tracing::info!("[{}] Proof generation complete (node {})", sid, node_id);
            }
            Err(e) => {
                session.status = SessionStatus::Failed(e.clone());
                tracing::error!("[{}] Proof generation failed: {}", sid, e);
            }
        }
    });

    Ok(StatusCode::ACCEPTED)
}

/// GET /session/:id/status
///
/// Poll for session status.
pub async fn get_status(
    State(state): State<NodeState>,
    Path(session_id): Path<String>,
) -> Result<Json<StatusResponse>, StatusCode> {
    let sessions = state.sessions.read().await;
    let session_lock = sessions.get(&session_id).ok_or(StatusCode::NOT_FOUND)?;
    let session = session_lock.read().await;

    let status_str = match &session.status {
        SessionStatus::SharesReceived => "shares_received".to_string(),
        SessionStatus::WitnessGenerating => "witness_generating".to_string(),
        SessionStatus::ProofGenerating => "proof_generating".to_string(),
        SessionStatus::Complete => "complete".to_string(),
        SessionStatus::Failed(e) => format!("failed: {}", e),
    };

    Ok(Json(StatusResponse {
        session_id: session.session_id.clone(),
        status: status_str,
    }))
}

/// GET /session/:id/proof
///
/// Retrieve the generated proof bytes (base64-encoded).
pub async fn get_proof(
    State(state): State<NodeState>,
    Path(session_id): Path<String>,
) -> Result<Json<ProofResponse>, (StatusCode, String)> {
    let sessions = state.sessions.read().await;
    let session_lock = sessions
        .get(&session_id)
        .ok_or((StatusCode::NOT_FOUND, "session not found".to_string()))?;
    let session = session_lock.read().await;

    if session.status != SessionStatus::Complete {
        return Err((StatusCode::BAD_REQUEST, format!("proof not ready, status: {:?}", session.status)));
    }

    let proof_bytes = session::get_proof(&session)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    use base64::Engine;
    let proof_b64 = base64::engine::general_purpose::STANDARD.encode(&proof_bytes);

    Ok(Json(ProofResponse {
        session_id: session.session_id.clone(),
        proof: proof_b64,
    }))
}

#[derive(Serialize)]
pub struct ProofResponse {
    pub session_id: String,
    pub proof: String, // base64-encoded proof bytes
}
