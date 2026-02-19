//! REST API handlers for the coordinator service.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use std::collections::HashMap;

use crate::{crypto, deck, hand_eval, mpc, soroban, AppState, TableSession};

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

/// POST /api/table/{table_id}/request-deal
///
/// Shuffle deck, deal hole cards, generate MPC deal proof.
pub async fn request_deal(
    State(state): State<AppState>,
    Path(table_id): Path<u32>,
) -> Result<Json<DealResponse>, StatusCode> {
    let mut tables = state.tables.write().await;

    // Shuffle deck (coordinator generates, then shares via MPC)
    let deck_state = deck::shuffle_deck_dev();

    // Deal cards: 2 per player (assuming 2 players for now)
    let num_players = 2u32;
    let mut player_cards = HashMap::new();
    let mut dealt_indices = Vec::new();

    for p in 0..num_players {
        let idx1 = (p * 2) as u32;
        let idx2 = (p * 2 + 1) as u32;
        player_cards.insert(format!("player_{}", p), (idx1, idx2));
        dealt_indices.push(idx1);
        dealt_indices.push(idx2);
    }

    let player_indices: Vec<(u32, u32)> = (0..num_players)
        .map(|p| (p * 2, p * 2 + 1))
        .collect();

    // Generate deal proof via MPC (real co-noir orchestration)
    let proof_result = mpc::generate_deal_proof(
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

    // Compute real Poseidon2 hand commitments matching the circuit
    let hand_commitments: Vec<String> = (0..num_players)
        .map(|p| {
            let idx1 = (p * 2) as u32;
            let idx2 = (p * 2 + 1) as u32;
            deck::compute_hand_commitment(&deck_state, idx1, idx2)
        })
        .collect();

    // Submit deal proof to Soroban (non-blocking: log + continue on failure)
    let tx_hash = match soroban::submit_deal_proof(
        &state.soroban_config,
        table_id,
        &proof_result.proof,
        &proof_result.public_inputs.iter().map(|s| s.as_bytes()).collect::<Vec<_>>().concat(),
        &deck_state.merkle_root,
        &hand_commitments,
    )
    .await
    {
        Ok(hash) if !hash.is_empty() => {
            tracing::info!("Deal proof submitted to Soroban: {}", hash);
            Some(hash)
        }
        Ok(_) => None,
        Err(e) => {
            tracing::warn!("Failed to submit deal proof to Soroban: {}", e);
            None
        }
    };

    // Store session
    let session = TableSession {
        table_id,
        deck_order: Some(deck_state.cards),
        card_salts: Some(deck_state.salts),
        deck_root: Some(deck_state.merkle_root.clone()),
        player_cards,
        dealt_indices,
        board_indices: Vec::new(),
        phase: "preflop".to_string(),
    };
    tables.insert(table_id, session);

    Ok(Json(DealResponse {
        status: "dealt".to_string(),
        deck_root: deck_state.merkle_root,
        hand_commitments,
        proof_size: proof_result.proof.len(),
        session_id: proof_result.session_id,
        tx_hash,
    }))
}

/// POST /api/table/{table_id}/request-reveal/{phase}
///
/// Reveal community cards (flop=3, turn=1, river=1).
pub async fn request_reveal(
    State(state): State<AppState>,
    Path((table_id, phase)): Path<(u32, String)>,
) -> Result<Json<RevealResponse>, StatusCode> {
    let mut tables = state.tables.write().await;
    let session = tables
        .get_mut(&table_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let deck_cards = session.deck_order.as_ref().ok_or(StatusCode::BAD_REQUEST)?;
    let salts = session.card_salts.as_ref().ok_or(StatusCode::BAD_REQUEST)?;

    let num_to_reveal = match phase.as_str() {
        "flop" => 3,
        "turn" => 1,
        "river" => 1,
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    let new_indices = deck::next_card_indices(&session.dealt_indices, num_to_reveal);

    let revealed: Vec<u32> = new_indices
        .iter()
        .map(|&i| deck_cards[i as usize])
        .collect();

    // Generate reveal proof via MPC
    let proof_result = mpc::generate_reveal_proof(
        &state.mpc_config.node_endpoints,
        &state.mpc_config.circuit_dir,
        deck_cards,
        salts,
        &new_indices,
        &session.dealt_indices,
    )
    .await
    .map_err(|e| {
        tracing::error!("Reveal proof generation failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Submit reveal proof to Soroban (non-blocking)
    let tx_hash = match soroban::submit_reveal_proof(
        &state.soroban_config,
        table_id,
        &proof_result.proof,
        &proof_result.public_inputs.iter().map(|s| s.as_bytes()).collect::<Vec<_>>().concat(),
        &revealed,
        &new_indices,
    )
    .await
    {
        Ok(hash) if !hash.is_empty() => {
            tracing::info!("Reveal proof submitted to Soroban: {}", hash);
            Some(hash)
        }
        Ok(_) => None,
        Err(e) => {
            tracing::warn!("Failed to submit reveal proof to Soroban: {}", e);
            None
        }
    };

    // Update session
    session.dealt_indices.extend(&new_indices);
    session.board_indices.extend(&new_indices);
    session.phase = phase.clone();

    Ok(Json(RevealResponse {
        status: "revealed".to_string(),
        cards: revealed,
        proof_size: proof_result.proof.len(),
        session_id: proof_result.session_id,
        tx_hash,
    }))
}

/// POST /api/table/{table_id}/request-showdown
///
/// Determine winner: collect hole cards, evaluate hands, generate showdown proof.
pub async fn request_showdown(
    State(state): State<AppState>,
    Path(table_id): Path<u32>,
) -> Result<Json<ShowdownResponse>, StatusCode> {
    let tables = state.tables.read().await;
    let session = tables
        .get(&table_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let deck_cards = session.deck_order.as_ref().ok_or(StatusCode::BAD_REQUEST)?;
    let salts = session.card_salts.as_ref().ok_or(StatusCode::BAD_REQUEST)?;

    // Collect all players' hole cards
    let mut hole_cards: Vec<(u32, u32)> = Vec::new();
    let mut salt_pairs: Vec<(String, String)> = Vec::new();
    let mut hand_commitments: Vec<String> = Vec::new();
    let mut player_keys: Vec<String> = session.player_cards.keys().cloned().collect();
    player_keys.sort(); // deterministic ordering

    for key in &player_keys {
        let (idx1, idx2) = session.player_cards[key];
        let card1 = deck_cards[idx1 as usize];
        let card2 = deck_cards[idx2 as usize];
        hole_cards.push((card1, card2));
        salt_pairs.push((
            salts[idx1 as usize].clone(),
            salts[idx2 as usize].clone(),
        ));
        // Compute real Poseidon2 hand commitment
        let c1 = crypto::commit_card(card1, &salts[idx1 as usize]);
        let c2 = crypto::commit_card(card2, &salts[idx2 as usize]);
        let hc = crypto::commit_hand(c1, c2);
        hand_commitments.push(crypto::fr_to_decimal_string(&hc));
    }

    // Get board cards
    let board_cards: Vec<u32> = session
        .board_indices
        .iter()
        .map(|&i| deck_cards[i as usize])
        .collect();

    // Proper poker hand evaluation matching the circuit's evaluate_hand_rank
    let mut best_score = 0u32;
    let mut winner_index = 0u32;
    for (i, (c1, c2)) in hole_cards.iter().enumerate() {
        // Build 7-card hand: 2 hole cards + 5 board cards
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

    // Generate showdown proof via MPC
    let proof_result = mpc::generate_showdown_proof(
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

    // Submit showdown proof to Soroban (non-blocking)
    let tx_hash = match soroban::submit_showdown_proof(
        &state.soroban_config,
        table_id,
        &proof_result.proof,
        &proof_result.public_inputs.iter().map(|s| s.as_bytes()).collect::<Vec<_>>().concat(),
        &hole_cards,
    )
    .await
    {
        Ok(hash) if !hash.is_empty() => {
            tracing::info!("Showdown proof submitted to Soroban: {}", hash);
            Some(hash)
        }
        Ok(_) => None,
        Err(e) => {
            tracing::warn!("Failed to submit showdown proof to Soroban: {}", e);
            None
        }
    };

    let winner_key = &player_keys[winner_index as usize];

    Ok(Json(ShowdownResponse {
        status: "showdown_complete".to_string(),
        winner: winner_key.clone(),
        winner_index,
        proof_size: proof_result.proof.len(),
        session_id: proof_result.session_id,
        tx_hash,
    }))
}

/// GET /api/table/{table_id}/player/{address}/cards
///
/// Private endpoint: delivers hole cards to a specific player.
pub async fn get_player_cards(
    State(state): State<AppState>,
    Path((table_id, address)): Path<(u32, String)>,
) -> Result<Json<PlayerCardsResponse>, StatusCode> {
    let tables = state.tables.read().await;
    let session = tables
        .get(&table_id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let deck_cards = session.deck_order.as_ref().ok_or(StatusCode::BAD_REQUEST)?;
    let salts = session.card_salts.as_ref().ok_or(StatusCode::BAD_REQUEST)?;

    let player_key = format!("player_{}", address);
    let (idx1, idx2) = session
        .player_cards
        .get(&player_key)
        .ok_or(StatusCode::NOT_FOUND)?;

    let card1 = deck_cards[*idx1 as usize];
    let card2 = deck_cards[*idx2 as usize];
    let salt1 = salts[*idx1 as usize].clone();
    let salt2 = salts[*idx2 as usize].clone();

    Ok(Json(PlayerCardsResponse {
        card1,
        card2,
        salt1,
        salt2,
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
pub async fn committee_status(
    State(state): State<AppState>,
) -> Json<CommitteeStatusResponse> {
    let healthy = mpc::check_node_health(&state.mpc_config.node_endpoints).await;

    Json(CommitteeStatusResponse {
        nodes: state.mpc_config.node_endpoints.len(),
        healthy,
        status: "active".to_string(),
    })
}
