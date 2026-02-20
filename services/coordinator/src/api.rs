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
use uuid::Uuid;

use crate::{mpc, soroban, AppState, TableSession};

const MAX_PLAYERS: usize = 6;
const MIN_PLAYERS: usize = 2;
const AUTH_SKEW_SECS: i64 = 300;
const RATE_LIMIT_WINDOW_SECS: u64 = 60;
const RATE_LIMIT_MAX_REQUESTS: usize = 60;
// Proof size varies by circuit and transcript hasher — not hardcoded.

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
/// All MPC nodes prepare private deal contributions and exchange share fragments.
/// Coordinator triggers proof generation and parses public outputs from the proof.
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

    if state.mpc_config.node_endpoints.is_empty() {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    let prepared_deal = mpc::prepare_deal_from_nodes(
        &state.mpc_config.node_endpoints,
        &state.mpc_config.circuit_dir,
        table_id,
        &req.players,
    )
    .await
    .map_err(|e| {
        tracing::error!("Deal preparation failed: {}", e);
        StatusCode::BAD_GATEWAY
    })?;

    let proof_session_id = format!("table-{}-deal-{}", table_id, Uuid::new_v4());
    let deal_proof = mpc::generate_proof_from_share_sets(
        table_id,
        &prepared_deal.share_set_ids,
        &proof_session_id,
        "deal_valid",
        &state.mpc_config.circuit_dir,
        &state.mpc_config.node_endpoints,
    )
    .await
    .map_err(|e| {
        tracing::error!("Deal proof generation failed: {}", e);
        StatusCode::BAD_GATEWAY
    })?;

    let parsed_deal = parse_deal_outputs(&deal_proof.public_inputs, req.players.len()).map_err(
        |e| {
            tracing::error!("Deal public input parsing failed: {}", e);
            StatusCode::BAD_GATEWAY
        },
    )?;

    let tx_hash = soroban::submit_deal_proof(
        &state.soroban_config,
        table_id,
        &deal_proof.proof,
        &concat_public_inputs(&deal_proof.public_inputs),
        &parsed_deal.deck_root,
        &parsed_deal.hand_commitments,
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

    let player_card_positions: Vec<(u32, u32)> = (0..req.players.len())
        .map(|p| {
            (
                parsed_deal.dealt_indices[p * 2],
                parsed_deal.dealt_indices[p * 2 + 1],
            )
        })
        .collect();

    let session = TableSession {
        table_id,
        deck_root: parsed_deal.deck_root.clone(),
        hand_commitments: parsed_deal.hand_commitments.clone(),
        player_order: req.players,
        dealt_indices: parsed_deal.dealt_indices,
        player_card_positions,
        board_indices: Vec::new(),
        phase: "preflop".to_string(),
        deal_session_id: deal_proof.session_id.clone(),
        deal_tx_hash: tx_hash.clone(),
        reveal_tx_hashes: HashMap::new(),
        reveal_session_ids: HashMap::new(),
        revealed_cards_by_phase: HashMap::new(),
        showdown_tx_hash: None,
        showdown_session_id: None,
        showdown_result: None,
        proof_nonce: 0,
    };

    state.tables.write().await.insert(table_id, session);

    Ok(Json(DealResponse {
        status: "dealt".to_string(),
        deck_root: parsed_deal.deck_root,
        hand_commitments: parsed_deal.hand_commitments,
        proof_size: deal_proof.proof.len(),
        session_id: deal_proof.session_id,
        tx_hash,
    }))
}

/// POST /api/table/{table_id}/request-reveal/{phase}
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

    if state.mpc_config.node_endpoints.is_empty() {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

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

    if let Some(existing_hash) = session.reveal_tx_hashes.get(&phase) {
        let cards = session
            .revealed_cards_by_phase
            .get(&phase)
            .cloned()
            .unwrap_or_default();
        let session_id = session
            .reveal_session_ids
            .get(&phase)
            .cloned()
            .unwrap_or_default();
        return Ok(Json(RevealResponse {
            status: "revealed".to_string(),
            cards,
            proof_size: 0,
            session_id,
            tx_hash: Some(existing_hash.clone()),
        }));
    }

    let prepared_reveal = mpc::prepare_reveal_from_nodes(
        &state.mpc_config.node_endpoints,
        &state.mpc_config.circuit_dir,
        table_id,
        &phase,
        &session.dealt_indices,
        &session.deck_root,
    )
    .await
    .map_err(|e| {
        tracing::error!("Reveal preparation failed: {}", e);
        StatusCode::BAD_GATEWAY
    })?;

    let proof_session_id = next_proof_session_id(session, &format!("reveal-{}", phase));
    let reveal_proof = mpc::generate_proof_from_share_sets(
        table_id,
        &prepared_reveal.share_set_ids,
        &proof_session_id,
        "reveal_board_valid",
        &state.mpc_config.circuit_dir,
        &state.mpc_config.node_endpoints,
    )
    .await
    .map_err(|e| {
        tracing::error!("Reveal proof generation failed: {}", e);
        StatusCode::BAD_GATEWAY
    })?;

    let num_revealed = match phase.as_str() {
        "flop" => 3usize,
        "turn" => 1usize,
        "river" => 1usize,
        _ => return Err(StatusCode::BAD_REQUEST),
    };
    let parsed_reveal =
        parse_reveal_outputs(&reveal_proof.public_inputs, num_revealed).map_err(|e| {
            tracing::error!("Reveal public input parsing failed: {}", e);
            StatusCode::BAD_GATEWAY
        })?;

    let tx_hash = soroban::submit_reveal_proof(
        &state.soroban_config,
        table_id,
        &reveal_proof.proof,
        &concat_public_inputs(&reveal_proof.public_inputs),
        &parsed_reveal.cards,
        &parsed_reveal.indices,
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

    session
        .dealt_indices
        .extend(parsed_reveal.indices.iter().copied());
    session
        .board_indices
        .extend(parsed_reveal.indices.iter().copied());
    session.phase = phase.clone();
    if let Some(hash) = tx_hash.clone() {
        session.reveal_tx_hashes.insert(phase.clone(), hash);
    }
    session
        .reveal_session_ids
        .insert(phase.clone(), reveal_proof.session_id.clone());
    session
        .revealed_cards_by_phase
        .insert(phase.clone(), parsed_reveal.cards.clone());

    Ok(Json(RevealResponse {
        status: "revealed".to_string(),
        cards: parsed_reveal.cards,
        proof_size: reveal_proof.proof.len(),
        session_id: reveal_proof.session_id,
        tx_hash,
    }))
}

/// POST /api/table/{table_id}/request-showdown
pub async fn request_showdown(
    State(state): State<AppState>,
    Path(table_id): Path<u32>,
    headers: HeaderMap,
) -> Result<Json<ShowdownResponse>, StatusCode> {
    validate_table_id(table_id)?;

    enforce_rate_limit(&state, &headers, table_id, "request_showdown").await?;
    let auth =
        validate_signed_request(&state, &headers, table_id, "request_showdown", None).await?;

    if state.mpc_config.node_endpoints.is_empty() {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }

    let mut tables = state.tables.write().await;
    let session = tables.get_mut(&table_id).ok_or(StatusCode::NOT_FOUND)?;

    if !session.player_order.iter().any(|p| p == &auth.address) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    if session.phase == "settlement" {
        if let Some((winner, winner_index)) = &session.showdown_result {
            return Ok(Json(ShowdownResponse {
                status: "showdown_complete".to_string(),
                winner: winner.clone(),
                winner_index: *winner_index,
                proof_size: 0,
                session_id: session.showdown_session_id.clone().unwrap_or_default(),
                tx_hash: session.showdown_tx_hash.clone(),
            }));
        }
        return Err(StatusCode::CONFLICT);
    }

    if session.phase != "river" {
        return Err(StatusCode::CONFLICT);
    }

    let prepared_showdown = mpc::prepare_showdown_from_nodes(
        &state.mpc_config.node_endpoints,
        &state.mpc_config.circuit_dir,
        table_id,
        &session.board_indices,
        session.player_order.len() as u32,
        &session.hand_commitments,
        &session.deck_root,
    )
    .await
    .map_err(|e| {
        tracing::error!("Showdown preparation failed: {}", e);
        StatusCode::BAD_GATEWAY
    })?;

    let proof_session_id = next_proof_session_id(session, "showdown");
    let showdown_proof = mpc::generate_proof_from_share_sets(
        table_id,
        &prepared_showdown.share_set_ids,
        &proof_session_id,
        "showdown_valid",
        &state.mpc_config.circuit_dir,
        &state.mpc_config.node_endpoints,
    )
    .await
    .map_err(|e| {
        tracing::error!("Showdown proof generation failed: {}", e);
        StatusCode::BAD_GATEWAY
    })?;

    let parsed_showdown = parse_showdown_outputs(
        &showdown_proof.public_inputs,
        session.player_order.len(),
    )
    .map_err(|e| {
        tracing::error!("Showdown public input parsing failed: {}", e);
        StatusCode::BAD_GATEWAY
    })?;

    if parsed_showdown.winner_index as usize >= session.player_order.len() {
        tracing::error!(
            "Showdown winner index out of range: {} >= {}",
            parsed_showdown.winner_index,
            session.player_order.len()
        );
        return Err(StatusCode::BAD_GATEWAY);
    }
    let winner = session.player_order[parsed_showdown.winner_index as usize].clone();

    let tx_hash = soroban::submit_showdown_proof(
        &state.soroban_config,
        table_id,
        &showdown_proof.proof,
        &concat_public_inputs(&showdown_proof.public_inputs),
        &parsed_showdown.hole_cards,
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
    session.showdown_tx_hash = tx_hash.clone();
    session.showdown_session_id = Some(showdown_proof.session_id.clone());
    session.showdown_result = Some((winner.clone(), parsed_showdown.winner_index));

    Ok(Json(ShowdownResponse {
        status: "showdown_complete".to_string(),
        winner,
        winner_index: parsed_showdown.winner_index,
        proof_size: showdown_proof.proof.len(),
        session_id: showdown_proof.session_id,
        tx_hash,
    }))
}

/// GET /api/table/{table_id}/player/{address}/cards
///
/// Resolve and return a player's hole cards by chaining permutation lookups
/// across MPC nodes.
pub async fn get_player_cards(
    State(state): State<AppState>,
    Path((table_id, address)): Path<(u32, String)>,
    headers: HeaderMap,
) -> Result<Json<PlayerCardsResponse>, StatusCode> {
    validate_table_id(table_id)?;
    let auth =
        validate_signed_request(&state, &headers, table_id, "get_player_cards", Some(&address))
            .await?;

    let tables = state.tables.read().await;
    let session = tables.get(&table_id).ok_or(StatusCode::NOT_FOUND)?;

    if !session.player_order.iter().any(|p| p == &auth.address) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let player_index = session
        .player_order
        .iter()
        .position(|p| p == &address)
        .ok_or(StatusCode::NOT_FOUND)?;

    let (pos1, pos2) = session
        .player_card_positions
        .get(player_index)
        .ok_or(StatusCode::NOT_FOUND)?;

    let node_endpoints = state.mpc_config.node_endpoints.clone();
    let positions = vec![*pos1, *pos2];
    drop(tables); // release read lock before async call

    let (cards, salts) = mpc::resolve_hole_cards(&node_endpoints, table_id, &positions)
        .await
        .map_err(|e| {
            tracing::error!("Failed to resolve hole cards: {}", e);
            StatusCode::BAD_GATEWAY
        })?;

    if cards.len() < 2 || salts.len() < 2 {
        return Err(StatusCode::BAD_GATEWAY);
    }

    Ok(Json(PlayerCardsResponse {
        card1: cards[0],
        card2: cards[1],
        salt1: salts[0].clone(),
        salt2: salts[1].clone(),
    }))
}

/// GET /api/table/{table_id}/state
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

struct ParsedDealOutputs {
    deck_root: String,
    hand_commitments: Vec<String>,
    dealt_indices: Vec<u32>,
}

struct ParsedRevealOutputs {
    cards: Vec<u32>,
    indices: Vec<u32>,
}

struct ParsedShowdownOutputs {
    hole_cards: Vec<(u32, u32)>,
    winner_index: u32,
}

fn parse_deal_outputs(public_inputs: &[String], num_players: usize) -> Result<ParsedDealOutputs, String> {
    let needed = 1 + MAX_PLAYERS + MAX_PLAYERS + MAX_PLAYERS;
    if public_inputs.len() < needed {
        return Err(format!(
            "deal public input vector too short: got {}, need at least {}",
            public_inputs.len(),
            needed
        ));
    }

    let start = public_inputs.len() - needed;
    let deck_root = public_inputs[start].clone();
    let hand_commitments = public_inputs[(start + 1)..(start + 1 + MAX_PLAYERS)].to_vec();

    let dealt1_slice =
        &public_inputs[(start + 1 + MAX_PLAYERS)..(start + 1 + 2 * MAX_PLAYERS)];
    let dealt2_slice =
        &public_inputs[(start + 1 + 2 * MAX_PLAYERS)..(start + 1 + 3 * MAX_PLAYERS)];
    let dealt1 = parse_u32_slice(dealt1_slice)?;
    let dealt2 = parse_u32_slice(dealt2_slice)?;

    if num_players > MAX_PLAYERS {
        return Err(format!("num_players {} exceeds MAX_PLAYERS {}", num_players, MAX_PLAYERS));
    }

    let mut dealt_indices = Vec::with_capacity(num_players * 2);
    for p in 0..num_players {
        dealt_indices.push(dealt1[p]);
        dealt_indices.push(dealt2[p]);
    }

    Ok(ParsedDealOutputs {
        deck_root,
        hand_commitments: hand_commitments[..num_players].to_vec(),
        dealt_indices,
    })
}

fn parse_reveal_outputs(
    public_inputs: &[String],
    num_revealed: usize,
) -> Result<ParsedRevealOutputs, String> {
    const MAX_REVEAL: usize = 3;
    let needed = MAX_REVEAL + MAX_REVEAL;
    if public_inputs.len() < needed {
        return Err(format!(
            "reveal public input vector too short: got {}, need at least {}",
            public_inputs.len(),
            needed
        ));
    }
    if num_revealed > MAX_REVEAL {
        return Err(format!(
            "num_revealed {} exceeds MAX_REVEAL {}",
            num_revealed, MAX_REVEAL
        ));
    }

    let start = public_inputs.len() - needed;
    let cards_all = parse_u32_slice(&public_inputs[start..(start + MAX_REVEAL)])?;
    let indices_all = parse_u32_slice(&public_inputs[(start + MAX_REVEAL)..(start + 2 * MAX_REVEAL)])?;

    Ok(ParsedRevealOutputs {
        cards: cards_all[..num_revealed].to_vec(),
        indices: indices_all[..num_revealed].to_vec(),
    })
}

fn parse_showdown_outputs(
    public_inputs: &[String],
    num_players: usize,
) -> Result<ParsedShowdownOutputs, String> {
    let needed = MAX_PLAYERS + MAX_PLAYERS + 1;
    if public_inputs.len() < needed {
        return Err(format!(
            "showdown public input vector too short: got {}, need at least {}",
            public_inputs.len(),
            needed
        ));
    }
    if num_players > MAX_PLAYERS {
        return Err(format!("num_players {} exceeds MAX_PLAYERS {}", num_players, MAX_PLAYERS));
    }

    let start = public_inputs.len() - needed;
    let hole1 = parse_u32_slice(&public_inputs[start..(start + MAX_PLAYERS)])?;
    let hole2 =
        parse_u32_slice(&public_inputs[(start + MAX_PLAYERS)..(start + 2 * MAX_PLAYERS)])?;
    let winner_index = parse_single_u32(&public_inputs[start + 2 * MAX_PLAYERS])?;

    let hole_cards = (0..num_players)
        .map(|i| (hole1[i], hole2[i]))
        .collect::<Vec<_>>();

    Ok(ParsedShowdownOutputs {
        hole_cards,
        winner_index,
    })
}

fn parse_u32_slice(raw: &[String]) -> Result<Vec<u32>, String> {
    raw.iter().map(|s| parse_single_u32(s)).collect()
}

fn parse_single_u32(raw: &str) -> Result<u32, String> {
    raw.parse::<u32>()
        .map_err(|e| format!("failed to parse '{}' as u32: {}", raw, e))
}

fn validate_table_id(_table_id: u32) -> Result<(), StatusCode> {
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

fn concat_public_inputs(public_inputs: &[String]) -> Vec<u8> {
    public_inputs
        .iter()
        .map(|s| s.as_bytes())
        .collect::<Vec<_>>()
        .concat()
}

fn next_proof_session_id(session: &mut TableSession, label: &str) -> String {
    session.proof_nonce = session.proof_nonce.saturating_add(1);
    format!(
        "table-{}-{}-{}",
        session.table_id, label, session.proof_nonce
    )
}
