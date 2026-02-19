//! REST API handlers for the coordinator service.

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use base64::Engine;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{
    deck, hand_eval, mpc, soroban, AppState, PlayerPrivateCards, PreparedReveal, PreparedShowdown,
    TableSession,
};

const MAX_PLAYERS: usize = 6;
const MIN_PLAYERS: usize = 2;
const AUTH_SKEW_SECS: i64 = 300;
const RATE_LIMIT_WINDOW_SECS: u64 = 60;
const RATE_LIMIT_MAX_REQUESTS: usize = 60;
const PROOF_BYTES: usize = 14_592;

#[derive(Deserialize)]
pub struct DealRequest {
    pub players: Vec<String>,
}

#[derive(Serialize)]
pub struct DealResponse {
    pub status: String,
    pub deck_root: String,
    pub hand_commitments: Vec<String>,
    pub proof_size: usize,
    pub session_id: String,
    pub tx_hash: Option<String>,
}

#[derive(Serialize)]
pub struct RevealResponse {
    pub status: String,
    pub cards: Vec<u32>,
    pub proof_size: usize,
    pub session_id: String,
    pub tx_hash: Option<String>,
}

#[derive(Serialize)]
pub struct ShowdownResponse {
    pub status: String,
    pub winner: String,
    pub winner_index: u32,
    pub proof_size: usize,
    pub session_id: String,
    pub tx_hash: Option<String>,
}

#[derive(Serialize)]
pub struct TableStateResponse {
    pub state: String,
}

#[derive(Serialize)]
pub struct PlayerCardsResponse {
    pub card1: u32,
    pub card2: u32,
    pub salt1: String,
    pub salt2: String,
}

#[derive(Serialize)]
pub struct CommitteeStatusResponse {
    pub nodes: usize,
    pub healthy: Vec<bool>,
    pub status: String,
}

struct AuthContext {
    address: String,
}

/// POST /api/table/{table_id}/request-deal
///
/// Shuffle deck, deal hole cards, pre-generate reveal/showdown proofs, and submit the
/// deal proof on-chain atomically.
pub async fn request_deal(
    State(state): State<AppState>,
    Path(table_id): Path<u32>,
    headers: HeaderMap,
    Json(req): Json<DealRequest>,
) -> Result<Json<DealResponse>, StatusCode> {
    validate_table_id(table_id)?;
    enforce_rate_limit(&state, &headers, table_id, "request_deal").await?;
    let auth = validate_signed_request(&state, &headers, table_id, "request_deal", None).await?;

    validate_players(&req.players)?;
    if !req.players.iter().any(|p| p == &auth.address) {
        tracing::warn!(
            "request_deal denied: caller {} is not in provided players list",
            auth.address
        );
        return Err(StatusCode::UNAUTHORIZED);
    }

    {
        let tables = state.tables.read().await;
        if let Some(existing) = tables.get(&table_id) {
            if existing.phase != "waiting" && existing.phase != "settlement" {
                return Err(StatusCode::CONFLICT);
            }
        }
    }

    // Shuffle deck (coordinator generates, then shares via MPC). This value is intentionally
    // not persisted in TableSession; only commitments and revealed per-player secrets are kept.
    let deck_state = deck::shuffle_deck_dev();

    let mut player_private_cards = HashMap::new();
    let mut dealt_indices = Vec::new();
    let mut player_indices: Vec<(u32, u32)> = Vec::new();

    for (seat, address) in req.players.iter().enumerate() {
        let idx1 = (seat as u32) * 2;
        let idx2 = idx1 + 1;

        player_private_cards.insert(
            address.clone(),
            PlayerPrivateCards {
                card1: deck_state.cards[idx1 as usize],
                card2: deck_state.cards[idx2 as usize],
                salt1: deck_state.salts[idx1 as usize].clone(),
                salt2: deck_state.salts[idx2 as usize].clone(),
            },
        );

        dealt_indices.push(idx1);
        dealt_indices.push(idx2);
        player_indices.push((idx1, idx2));
    }

    // Generate deal proof via MPC.
    let deal_proof = mpc::generate_deal_proof(
        &state.mpc_config.node_endpoints,
        &state.mpc_config.circuit_dir,
        &deck_state.cards,
        &deck_state.salts,
        &player_indices,
    )
    .await
    .map_err(|e| {
        tracing::error!("Deal proof generation failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if deal_proof.proof.len() != PROOF_BYTES {
        tracing::error!(
            "Deal proof size mismatch: got {} bytes, expected {}",
            deal_proof.proof.len(),
            PROOF_BYTES
        );
        return Err(StatusCode::BAD_GATEWAY);
    }

    // Compute hand commitments in seat order.
    let hand_commitments: Vec<String> = player_indices
        .iter()
        .map(|(idx1, idx2)| deck::compute_hand_commitment(&deck_state, *idx1, *idx2))
        .collect();

    // Pre-generate all reveal proofs now, so the coordinator does not persist deck plaintext.
    let mut reveal_plans: HashMap<String, PreparedReveal> = HashMap::new();
    let mut used_indices = dealt_indices.clone();
    let mut full_board_indices: Vec<u32> = Vec::new();

    for (phase, count) in [("flop", 3usize), ("turn", 1usize), ("river", 1usize)] {
        let indices = deck::next_card_indices(&used_indices, count);
        let cards: Vec<u32> = indices
            .iter()
            .map(|&i| deck_state.cards[i as usize])
            .collect();

        let reveal_proof = mpc::generate_reveal_proof(
            &state.mpc_config.node_endpoints,
            &state.mpc_config.circuit_dir,
            &deck_state.cards,
            &deck_state.salts,
            &indices,
            &used_indices,
        )
        .await
        .map_err(|e| {
            tracing::error!("{} proof generation failed: {}", phase, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        if reveal_proof.proof.len() != PROOF_BYTES {
            tracing::error!(
                "{} proof size mismatch: got {} bytes, expected {}",
                phase,
                reveal_proof.proof.len(),
                PROOF_BYTES
            );
            return Err(StatusCode::BAD_GATEWAY);
        }

        reveal_plans.insert(
            phase.to_string(),
            PreparedReveal {
                cards: cards.clone(),
                indices: indices.clone(),
                proof: reveal_proof.proof,
                public_inputs: reveal_proof
                    .public_inputs
                    .iter()
                    .map(|s| s.as_bytes())
                    .collect::<Vec<_>>()
                    .concat(),
                session_id: reveal_proof.session_id,
                submitted_tx_hash: None,
            },
        );

        used_indices.extend(indices.iter().copied());
        full_board_indices.extend(indices);
    }

    // Pre-compute showdown payload.
    let board_cards: Vec<u32> = full_board_indices
        .iter()
        .map(|&i| deck_state.cards[i as usize])
        .collect();

    let mut hole_cards: Vec<(u32, u32)> = Vec::new();
    let mut salt_pairs: Vec<(String, String)> = Vec::new();
    for address in &req.players {
        let cards = player_private_cards
            .get(address)
            .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
        hole_cards.push((cards.card1, cards.card2));
        salt_pairs.push((cards.salt1.clone(), cards.salt2.clone()));
    }

    let mut best_score = 0u32;
    let mut winner_index = 0u32;
    for (i, (c1, c2)) in hole_cards.iter().enumerate() {
        let mut seven_cards = [0u32; 7];
        seven_cards[0] = *c1;
        seven_cards[1] = *c2;
        for (j, &bc) in board_cards.iter().enumerate().take(5) {
            seven_cards[2 + j] = bc;
        }
        let score = hand_eval::evaluate_hand_rank(&seven_cards);
        if score > best_score {
            best_score = score;
            winner_index = i as u32;
        }
    }

    let showdown_proof = mpc::generate_showdown_proof(
        &state.mpc_config.node_endpoints,
        &state.mpc_config.circuit_dir,
        &hole_cards,
        &board_cards,
        &salt_pairs,
        &hand_commitments,
        winner_index,
    )
    .await
    .map_err(|e| {
        tracing::error!("Showdown proof generation failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if showdown_proof.proof.len() != PROOF_BYTES {
        tracing::error!(
            "Showdown proof size mismatch: got {} bytes, expected {}",
            showdown_proof.proof.len(),
            PROOF_BYTES
        );
        return Err(StatusCode::BAD_GATEWAY);
    }

    // Atomic submission rule: if submission fails, return error and do not persist session.
    let tx_hash = soroban::submit_deal_proof(
        &state.soroban_config,
        table_id,
        &deal_proof.proof,
        &deal_proof
            .public_inputs
            .iter()
            .map(|s| s.as_bytes())
            .collect::<Vec<_>>()
            .concat(),
        &deck_state.merkle_root,
        &hand_commitments,
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to submit deal proof to Soroban: {}", e);
        StatusCode::BAD_GATEWAY
    })?;

    let tx_hash = if tx_hash.is_empty() {
        None
    } else {
        Some(tx_hash)
    };

    let showdown_plan = PreparedShowdown {
        winner: req.players[winner_index as usize].clone(),
        winner_index,
        hole_cards,
        proof: showdown_proof.proof,
        public_inputs: showdown_proof
            .public_inputs
            .iter()
            .map(|s| s.as_bytes())
            .collect::<Vec<_>>()
            .concat(),
        session_id: showdown_proof.session_id,
        submitted_tx_hash: None,
    };

    let session = TableSession {
        table_id,
        deck_commitments: deck_state.commitments,
        deck_root: deck_state.merkle_root.clone(),
        player_cards: player_private_cards,
        player_order: req.players,
        dealt_indices,
        board_indices: Vec::new(),
        reveal_plans,
        showdown_plan,
        phase: "preflop".to_string(),
    };

    state.tables.write().await.insert(table_id, session);

    Ok(Json(DealResponse {
        status: "dealt".to_string(),
        deck_root: deck_state.merkle_root,
        hand_commitments,
        proof_size: deal_proof.proof.len(),
        session_id: deal_proof.session_id,
        tx_hash,
    }))
}

/// POST /api/table/{table_id}/request-reveal/{phase}
///
/// Reveal community cards (flop=3, turn=1, river=1).
pub async fn request_reveal(
    State(state): State<AppState>,
    Path((table_id, phase)): Path<(u32, String)>,
    headers: HeaderMap,
) -> Result<Json<RevealResponse>, StatusCode> {
    validate_table_id(table_id)?;
    validate_reveal_phase(&phase)?;

    let action = format!("request_reveal:{}", phase);
    enforce_rate_limit(&state, &headers, table_id, &action).await?;
    let auth = validate_signed_request(&state, &headers, table_id, &action, None).await?;

    let mut tables = state.tables.write().await;
    let session = tables.get_mut(&table_id).ok_or(StatusCode::NOT_FOUND)?;

    if !session.player_order.iter().any(|p| p == &auth.address) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let expected_next_phase = match session.phase.as_str() {
        "preflop" => "flop",
        "flop" => "turn",
        "turn" => "river",
        _ => return Err(StatusCode::CONFLICT),
    };

    if phase != expected_next_phase {
        return Err(StatusCode::CONFLICT);
    }

    let plan = session
        .reveal_plans
        .get_mut(&phase)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(existing_hash) = &plan.submitted_tx_hash {
        return Ok(Json(RevealResponse {
            status: "revealed".to_string(),
            cards: plan.cards.clone(),
            proof_size: plan.proof.len(),
            session_id: plan.session_id.clone(),
            tx_hash: Some(existing_hash.clone()),
        }));
    }

    // Atomic submission rule: if on-chain submission fails, do not mutate session phase.
    let tx_hash = soroban::submit_reveal_proof(
        &state.soroban_config,
        table_id,
        &plan.proof,
        &plan.public_inputs,
        &plan.cards,
        &plan.indices,
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to submit reveal proof to Soroban: {}", e);
        StatusCode::BAD_GATEWAY
    })?;

    let tx_hash = if tx_hash.is_empty() {
        None
    } else {
        Some(tx_hash)
    };

    session.dealt_indices.extend(plan.indices.iter().copied());
    session.board_indices.extend(plan.indices.iter().copied());
    session.phase = phase.clone();
    plan.submitted_tx_hash = tx_hash.clone();

    Ok(Json(RevealResponse {
        status: "revealed".to_string(),
        cards: plan.cards.clone(),
        proof_size: plan.proof.len(),
        session_id: plan.session_id.clone(),
        tx_hash,
    }))
}

/// POST /api/table/{table_id}/request-showdown
///
/// Determine winner and submit pre-generated showdown proof.
pub async fn request_showdown(
    State(state): State<AppState>,
    Path(table_id): Path<u32>,
    headers: HeaderMap,
) -> Result<Json<ShowdownResponse>, StatusCode> {
    validate_table_id(table_id)?;

    enforce_rate_limit(&state, &headers, table_id, "request_showdown").await?;
    let auth =
        validate_signed_request(&state, &headers, table_id, "request_showdown", None).await?;

    let mut tables = state.tables.write().await;
    let session = tables.get_mut(&table_id).ok_or(StatusCode::NOT_FOUND)?;

    if !session.player_order.iter().any(|p| p == &auth.address) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    if session.phase != "river" {
        return Err(StatusCode::CONFLICT);
    }

    if let Some(existing_hash) = &session.showdown_plan.submitted_tx_hash {
        return Ok(Json(ShowdownResponse {
            status: "showdown_complete".to_string(),
            winner: session.showdown_plan.winner.clone(),
            winner_index: session.showdown_plan.winner_index,
            proof_size: session.showdown_plan.proof.len(),
            session_id: session.showdown_plan.session_id.clone(),
            tx_hash: Some(existing_hash.clone()),
        }));
    }

    // Atomic submission rule: if this fails, keep phase as-is.
    let tx_hash = soroban::submit_showdown_proof(
        &state.soroban_config,
        table_id,
        &session.showdown_plan.proof,
        &session.showdown_plan.public_inputs,
        &session.showdown_plan.hole_cards,
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to submit showdown proof to Soroban: {}", e);
        StatusCode::BAD_GATEWAY
    })?;

    let tx_hash = if tx_hash.is_empty() {
        None
    } else {
        Some(tx_hash)
    };

    session.phase = "settlement".to_string();
    session.showdown_plan.submitted_tx_hash = tx_hash.clone();

    Ok(Json(ShowdownResponse {
        status: "showdown_complete".to_string(),
        winner: session.showdown_plan.winner.clone(),
        winner_index: session.showdown_plan.winner_index,
        proof_size: session.showdown_plan.proof.len(),
        session_id: session.showdown_plan.session_id.clone(),
        tx_hash,
    }))
}

/// GET /api/table/{table_id}/player/{address}/cards
///
/// Private endpoint: delivers hole cards to the authenticated player.
pub async fn get_player_cards(
    State(state): State<AppState>,
    Path((table_id, address)): Path<(u32, String)>,
    headers: HeaderMap,
) -> Result<Json<PlayerCardsResponse>, StatusCode> {
    validate_table_id(table_id)?;

    enforce_rate_limit(&state, &headers, table_id, "get_player_cards").await?;
    let auth = validate_signed_request(
        &state,
        &headers,
        table_id,
        "get_player_cards",
        Some(&address),
    )
    .await?;

    let tables = state.tables.read().await;
    let session = tables.get(&table_id).ok_or(StatusCode::NOT_FOUND)?;

    if !session.player_order.iter().any(|p| p == &auth.address) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let cards = session
        .player_cards
        .get(&address)
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(PlayerCardsResponse {
        card1: cards.card1,
        card2: cards.card2,
        salt1: cards.salt1.clone(),
        salt2: cards.salt2.clone(),
    }))
}

/// GET /api/table/{table_id}/state
///
/// Read on-chain table state via Soroban.
pub async fn get_table_state(
    State(state): State<AppState>,
    Path(table_id): Path<u32>,
) -> Result<Json<TableStateResponse>, StatusCode> {
    let result = soroban::get_table_state(&state.soroban_config, table_id)
        .await
        .map_err(|e| {
            tracing::error!("Failed to read table state: {}", e);
            StatusCode::SERVICE_UNAVAILABLE
        })?;

    Ok(Json(TableStateResponse { state: result }))
}

/// GET /api/committee/status
pub async fn committee_status(State(state): State<AppState>) -> Json<CommitteeStatusResponse> {
    let healthy = mpc::check_node_health(&state.mpc_config.node_endpoints).await;

    Json(CommitteeStatusResponse {
        nodes: state.mpc_config.node_endpoints.len(),
        healthy,
        status: "active".to_string(),
    })
}

fn validate_table_id(table_id: u32) -> Result<(), StatusCode> {
    if table_id == 0 {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

fn validate_players(players: &[String]) -> Result<(), StatusCode> {
    if players.len() < MIN_PLAYERS || players.len() > MAX_PLAYERS {
        return Err(StatusCode::BAD_REQUEST);
    }

    let mut seen = HashSet::new();
    for address in players {
        if !is_valid_stellar_address(address) {
            return Err(StatusCode::BAD_REQUEST);
        }
        if !seen.insert(address) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    Ok(())
}

fn validate_reveal_phase(phase: &str) -> Result<(), StatusCode> {
    match phase {
        "flop" | "turn" | "river" => Ok(()),
        _ => Err(StatusCode::BAD_REQUEST),
    }
}

async fn enforce_rate_limit(
    state: &AppState,
    headers: &HeaderMap,
    table_id: u32,
    action: &str,
) -> Result<(), StatusCode> {
    let now = now_unix_secs_u64()?;
    let ip = extract_ip(headers);
    let bucket_key = format!("{}:{}:{}", ip, table_id, action);

    let mut rl = state.rate_limit_state.write().await;
    let bucket = rl.requests_by_bucket.entry(bucket_key).or_default();

    bucket.retain(|ts| now.saturating_sub(*ts) <= RATE_LIMIT_WINDOW_SECS);
    if bucket.len() >= RATE_LIMIT_MAX_REQUESTS {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    bucket.push(now);
    Ok(())
}

async fn validate_signed_request(
    state: &AppState,
    headers: &HeaderMap,
    table_id: u32,
    action: &str,
    expected_address: Option<&str>,
) -> Result<AuthContext, StatusCode> {
    let address = header_string(headers, "x-player-address")?;
    let signature_raw = header_string(headers, "x-auth-signature")?;
    let nonce = header_string(headers, "x-auth-nonce")
        .and_then(|v| v.parse::<u64>().map_err(|_| StatusCode::UNAUTHORIZED))?;
    let timestamp = header_string(headers, "x-auth-timestamp")
        .and_then(|v| v.parse::<i64>().map_err(|_| StatusCode::UNAUTHORIZED))?;

    if let Some(expected) = expected_address {
        if expected != address {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    if !is_valid_stellar_address(&address) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let now = now_unix_secs_i64()?;
    if (now - timestamp).abs() > AUTH_SKEW_SECS {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let message = auth_message(&address, table_id, action, nonce, timestamp);
    verify_signature(&address, &message, &signature_raw)?;

    // Replay protection: require strictly increasing nonce per wallet address.
    let mut auth_state = state.auth_state.write().await;
    if let Some(last_nonce) = auth_state.last_nonce_by_address.get(&address) {
        if nonce <= *last_nonce {
            return Err(StatusCode::CONFLICT);
        }
    }
    auth_state
        .last_nonce_by_address
        .insert(address.clone(), nonce);

    Ok(AuthContext { address })
}

fn verify_signature(address: &str, message: &str, signature_raw: &str) -> Result<(), StatusCode> {
    let stellar_pk = stellar_strkey::ed25519::PublicKey::from_string(address)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let verifying_key =
        VerifyingKey::from_bytes(&stellar_pk.0).map_err(|_| StatusCode::UNAUTHORIZED)?;

    let signature = decode_signature(signature_raw)?;
    verifying_key
        .verify(message.as_bytes(), &signature)
        .map_err(|_| StatusCode::UNAUTHORIZED)
}

fn decode_signature(signature_raw: &str) -> Result<Signature, StatusCode> {
    let s = signature_raw.trim();

    // Accept 64-byte hex (with or without 0x) and base64 to tolerate wallet format changes.
    let decoded = if let Some(hex) = s.strip_prefix("0x") {
        hex::decode(hex).map_err(|_| StatusCode::UNAUTHORIZED)?
    } else if s.len() == 128 && s.chars().all(|c| c.is_ascii_hexdigit()) {
        hex::decode(s).map_err(|_| StatusCode::UNAUTHORIZED)?
    } else {
        base64::engine::general_purpose::STANDARD
            .decode(s)
            .map_err(|_| StatusCode::UNAUTHORIZED)?
    };

    let bytes: [u8; 64] = decoded
        .as_slice()
        .try_into()
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    Ok(Signature::from_bytes(&bytes))
}

fn auth_message(address: &str, table_id: u32, action: &str, nonce: u64, timestamp: i64) -> String {
    format!(
        "stellar-poker|{}|{}|{}|{}|{}",
        address, table_id, action, nonce, timestamp
    )
}

fn header_string(headers: &HeaderMap, key: &str) -> Result<String, StatusCode> {
    headers
        .get(key)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_string())
        .ok_or(StatusCode::UNAUTHORIZED)
}

fn extract_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or("unknown").trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn is_valid_stellar_address(address: &str) -> bool {
    stellar_strkey::ed25519::PublicKey::from_string(address).is_ok()
}

fn now_unix_secs_u64() -> Result<u64, StatusCode> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(now.as_secs())
}

fn now_unix_secs_i64() -> Result<i64, StatusCode> {
    let now = now_unix_secs_u64()?;
    i64::try_from(now).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
