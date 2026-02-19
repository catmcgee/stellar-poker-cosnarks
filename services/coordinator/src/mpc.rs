//! MPC integration module for TACEO coNoir.
//!
//! Orchestrates the 3-party MPC protocol:
//! 1. Prepare Prover.toml with circuit inputs
//! 2. Run `co-noir split-input` to create REP3 secret shares
//! 3. Distribute shares to 3 MPC nodes via HTTP
//! 4. Trigger proof generation on all nodes
//! 5. Poll node 0 for the completed proof
//!
//! The resulting proof is a standard UltraHonk proof compatible with
//! any Barretenberg verifier (including our on-chain Soroban verifier).

use ark_bn254::Fr;
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;
use uuid::Uuid;

use crate::crypto;

/// Result from MPC proof generation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MpcProofResult {
    pub proof: Vec<u8>,
    pub public_inputs: Vec<String>,
    pub session_id: String,
}

/// Node status response for polling.
#[derive(Deserialize)]
struct NodeStatusResponse {
    #[allow(dead_code)]
    session_id: String,
    status: String,
}

/// Node proof response.
#[derive(Deserialize)]
struct NodeProofResponse {
    #[allow(dead_code)]
    session_id: String,
    proof: String, // base64
}

// ---------------------------------------------------------------------------
// Public proof generation functions
// ---------------------------------------------------------------------------

/// Generate a deal proof via MPC.
///
/// The coordinator:
/// 1. Writes a Prover.toml with the deal circuit inputs
/// 2. Runs co-noir split-input to create shares
/// 3. Distributes shares to nodes
/// 4. Triggers proof generation
/// 5. Collects the proof from node 0
pub async fn generate_deal_proof(
    node_endpoints: &[String],
    circuit_dir: &str,
    deck: &[u32],
    salts: &[String],
    player_card_indices: &[(u32, u32)],
) -> Result<MpcProofResult, String> {
    let session_id = Uuid::new_v4().to_string();
    let circuit_name = "deal_valid";

    tracing::info!("[{}] Generating deal proof via MPC", session_id);

    // Build Prover.toml content
    let prover_toml = build_deal_prover_toml(deck, salts, player_card_indices);

    // Run the MPC pipeline
    run_mpc_pipeline(
        &session_id,
        circuit_name,
        circuit_dir,
        &prover_toml,
        node_endpoints,
    )
    .await
}

/// Generate a board reveal proof via MPC.
pub async fn generate_reveal_proof(
    node_endpoints: &[String],
    circuit_dir: &str,
    deck: &[u32],
    salts: &[String],
    reveal_indices: &[u32],
    previously_used: &[u32],
) -> Result<MpcProofResult, String> {
    let session_id = Uuid::new_v4().to_string();
    let circuit_name = "reveal_board_valid";

    tracing::info!(
        "[{}] Generating reveal proof via MPC for {} cards",
        session_id,
        reveal_indices.len()
    );

    let prover_toml = build_reveal_prover_toml(deck, salts, reveal_indices, previously_used);

    run_mpc_pipeline(
        &session_id,
        circuit_name,
        circuit_dir,
        &prover_toml,
        node_endpoints,
    )
    .await
}

/// Generate a showdown proof via MPC.
pub async fn generate_showdown_proof(
    node_endpoints: &[String],
    circuit_dir: &str,
    hole_cards: &[(u32, u32)],
    board_cards: &[u32],
    salts: &[(String, String)],
    hand_commitments: &[String],
    winner_index: u32,
) -> Result<MpcProofResult, String> {
    let session_id = Uuid::new_v4().to_string();
    let circuit_name = "showdown_valid";

    tracing::info!("[{}] Generating showdown proof via MPC", session_id);

    let prover_toml = build_showdown_prover_toml(
        hole_cards,
        board_cards,
        salts,
        hand_commitments,
        winner_index,
    );

    run_mpc_pipeline(
        &session_id,
        circuit_name,
        circuit_dir,
        &prover_toml,
        node_endpoints,
    )
    .await
}

/// Check health of all MPC nodes.
pub async fn check_node_health(endpoints: &[String]) -> Vec<bool> {
    let mut results = Vec::new();
    for endpoint in endpoints {
        let healthy = reqwest::get(format!("{}/health", endpoint))
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        results.push(healthy);
    }
    results
}

// ---------------------------------------------------------------------------
// MPC pipeline helpers
// ---------------------------------------------------------------------------

/// Core MPC pipeline: split inputs → distribute shares → trigger → collect proof.
async fn run_mpc_pipeline(
    session_id: &str,
    circuit_name: &str,
    circuit_dir: &str,
    prover_toml: &str,
    node_endpoints: &[String],
) -> Result<MpcProofResult, String> {
    let work_dir = tempfile::tempdir()
        .map_err(|e| format!("failed to create temp dir: {}", e))?;
    let work_path = work_dir.path();

    let crs_dir = std::env::var("CRS_DIR").unwrap_or_else(|_| "./crs".to_string());
    let crs_path = format!("{}/bn254_g1.dat", crs_dir);

    // Step 1: Write Prover.toml
    let prover_path = work_path.join("Prover.toml");
    std::fs::write(&prover_path, prover_toml)
        .map_err(|e| format!("failed to write Prover.toml: {}", e))?;

    // Step 2: Split inputs into REP3 shares
    let shares_dir = work_path.join("shares");
    std::fs::create_dir_all(&shares_dir)
        .map_err(|e| format!("failed to create shares dir: {}", e))?;

    split_input(circuit_dir, circuit_name, &prover_path, &shares_dir).await?;

    // Step 3: Distribute shares to nodes
    distribute_shares(session_id, circuit_name, &shares_dir, node_endpoints).await?;

    // Step 4: Trigger proof generation on all nodes and collect proof from node 0
    let proof_result = trigger_and_collect_proof(
        session_id,
        circuit_dir,
        &crs_path,
        node_endpoints,
    )
    .await?;

    Ok(proof_result)
}

/// Run `co-noir split-input` to create REP3 secret shares of the Prover.toml.
async fn split_input(
    circuit_dir: &str,
    circuit_name: &str,
    prover_path: &Path,
    shares_dir: &Path,
) -> Result<(), String> {
    let circuit_path = format!("{}/{}/target/{}.json", circuit_dir, circuit_name, circuit_name);

    let output = Command::new("co-noir")
        .arg("split-input")
        .arg("--circuit")
        .arg(&circuit_path)
        .arg("--input")
        .arg(prover_path)
        .arg("--protocol")
        .arg("REP3")
        .arg("--out-dir")
        .arg(shares_dir)
        .output()
        .await
        .map_err(|e| format!("failed to spawn co-noir split-input: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!("co-noir split-input failed:\nstderr: {}\nstdout: {}", stderr, stdout));
    }

    tracing::info!("Split input into 3 shares");
    Ok(())
}

/// Distribute share files to the 3 MPC nodes via HTTP.
async fn distribute_shares(
    session_id: &str,
    circuit_name: &str,
    shares_dir: &Path,
    node_endpoints: &[String],
) -> Result<(), String> {
    let client = reqwest::Client::new();

    for (i, endpoint) in node_endpoints.iter().enumerate() {
        let share_file = shares_dir.join(format!("party_{}.toml", i));
        let share_bytes = std::fs::read(&share_file)
            .map_err(|e| format!("failed to read share file for party {}: {}", i, e))?;

        let share_b64 = base64::engine::general_purpose::STANDARD.encode(&share_bytes);

        let url = format!("{}/session/{}/shares", endpoint, session_id);
        let resp = client
            .post(&url)
            .json(&serde_json::json!({
                "circuit_name": circuit_name,
                "share_data": share_b64,
            }))
            .send()
            .await
            .map_err(|e| format!("failed to send shares to node {}: {}", i, e))?;

        if !resp.status().is_success() {
            return Err(format!(
                "node {} rejected shares: HTTP {}",
                i,
                resp.status()
            ));
        }
    }

    tracing::info!("[{}] Shares distributed to {} nodes", session_id, node_endpoints.len());
    Ok(())
}

/// Trigger proof generation on all nodes and poll node 0 for the result.
async fn trigger_and_collect_proof(
    session_id: &str,
    circuit_dir: &str,
    crs_path: &str,
    node_endpoints: &[String],
) -> Result<MpcProofResult, String> {
    let client = reqwest::Client::new();

    // Trigger generation on all nodes concurrently
    let mut handles = Vec::new();
    for (i, endpoint) in node_endpoints.iter().enumerate() {
        let url = format!("{}/session/{}/generate", endpoint, session_id);
        let client = client.clone();
        let circuit_dir = circuit_dir.to_string();
        let crs_path = crs_path.to_string();
        let handle = tokio::spawn(async move {
            let resp = client
                .post(&url)
                .json(&serde_json::json!({
                    "circuit_dir": circuit_dir,
                    "crs_path": crs_path,
                }))
                .send()
                .await
                .map_err(|e| format!("failed to trigger node {}: {}", i, e))?;

            if !resp.status().is_success() {
                return Err(format!("node {} trigger failed: HTTP {}", i, resp.status()));
            }
            Ok::<(), String>(())
        });
        handles.push(handle);
    }

    // Wait for all triggers to complete
    for handle in handles {
        handle.await.map_err(|e| format!("join error: {}", e))??;
    }

    tracing::info!("[{}] All nodes triggered, polling for proof...", session_id);

    // Poll node 0 for proof completion
    let proof_node = &node_endpoints[0];
    let max_polls = 300; // 5 minutes at 1s intervals
    for attempt in 0..max_polls {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        let status_url = format!("{}/session/{}/status", proof_node, session_id);
        let resp = client
            .get(&status_url)
            .send()
            .await
            .map_err(|e| format!("failed to poll node 0: {}", e))?;

        if !resp.status().is_success() {
            continue;
        }

        let status: NodeStatusResponse = resp
            .json()
            .await
            .map_err(|e| format!("failed to parse status: {}", e))?;

        match status.status.as_str() {
            "complete" => {
                tracing::info!(
                    "[{}] Proof ready after {} seconds",
                    session_id,
                    attempt + 1
                );

                // Fetch the proof
                let proof_url = format!("{}/session/{}/proof", proof_node, session_id);
                let proof_resp = client
                    .get(&proof_url)
                    .send()
                    .await
                    .map_err(|e| format!("failed to fetch proof: {}", e))?;

                let proof_data: NodeProofResponse = proof_resp
                    .json()
                    .await
                    .map_err(|e| format!("failed to parse proof: {}", e))?;

                let proof_bytes = base64::engine::general_purpose::STANDARD
                    .decode(&proof_data.proof)
                    .map_err(|e| format!("failed to decode proof: {}", e))?;

                return Ok(MpcProofResult {
                    proof: proof_bytes,
                    public_inputs: vec![],
                    session_id: session_id.to_string(),
                });
            }
            s if s.starts_with("failed") => {
                return Err(format!("proof generation failed: {}", s));
            }
            _ => {
                if attempt % 10 == 0 {
                    tracing::debug!(
                        "[{}] Still waiting... status: {} (attempt {})",
                        session_id,
                        status.status,
                        attempt
                    );
                }
            }
        }
    }

    Err(format!(
        "[{}] Proof generation timed out after {} seconds",
        session_id, max_polls
    ))
}

// ---------------------------------------------------------------------------
// Prover.toml builders
// ---------------------------------------------------------------------------

/// Format a Noir Field array for Prover.toml.
fn format_field_array(name: &str, values: &[u32]) -> String {
    let items: Vec<String> = values.iter().map(|v| format!("\"{}\"", v)).collect();
    format!("{} = [{}]", name, items.join(", "))
}

/// Format a Noir Field array from string values.
fn format_field_array_str(name: &str, values: &[String]) -> String {
    let items: Vec<String> = values.iter().map(|v| format!("\"{}\"", v)).collect();
    format!("{} = [{}]", name, items.join(", "))
}

/// Build Prover.toml for deal_valid circuit with real Poseidon2 public inputs.
fn build_deal_prover_toml(
    deck: &[u32],
    salts: &[String],
    player_card_indices: &[(u32, u32)],
) -> String {
    let num_players = player_card_indices.len();
    let max_players = 6;

    let deck_items: Vec<String> = deck.iter().map(|c| format!("\"{}\"", c)).collect();
    let salts_items: Vec<String> = salts.iter().map(|s| format!("\"{}\"", s)).collect();

    // Compute real Poseidon2 commitments and Merkle root
    let commitment_frs: Vec<Fr> = deck
        .iter()
        .zip(salts.iter())
        .map(|(card, salt)| crypto::commit_card(*card, salt))
        .collect();

    let mut leaves = [Fr::from(0u64); 64];
    for (i, c) in commitment_frs.iter().enumerate() {
        leaves[i] = *c;
    }
    let deck_root = crypto::compute_merkle_root(&leaves);

    // Compute real hand commitments
    let mut card1_indices = vec![0u32; max_players];
    let mut card2_indices = vec![0u32; max_players];
    let mut hand_commitments = vec!["0".to_string(); max_players];

    for (i, (idx1, idx2)) in player_card_indices.iter().enumerate() {
        card1_indices[i] = *idx1;
        card2_indices[i] = *idx2;
        let c1 = commitment_frs[*idx1 as usize];
        let c2 = commitment_frs[*idx2 as usize];
        let hand = crypto::commit_hand(c1, c2);
        hand_commitments[i] = crypto::fr_to_decimal_string(&hand);
    }

    let mut lines = Vec::new();
    lines.push(format!("deck = [{}]", deck_items.join(", ")));
    lines.push(format!("salts = [{}]", salts_items.join(", ")));
    lines.push(format!("deck_root = \"{}\"", crypto::fr_to_decimal_string(&deck_root)));
    lines.push(format!("num_players = \"{}\"", num_players));
    lines.push(format_field_array_str("hand_commitments", &hand_commitments));
    lines.push(format_field_array("dealt_card1_indices", &card1_indices));
    lines.push(format_field_array("dealt_card2_indices", &card2_indices));

    lines.join("\n")
}

/// Build Prover.toml for reveal_board_valid circuit with real Poseidon2 root.
fn build_reveal_prover_toml(
    deck: &[u32],
    salts: &[String],
    reveal_indices: &[u32],
    previously_used: &[u32],
) -> String {
    let max_reveal = 3;
    let max_used = 16;

    let deck_items: Vec<String> = deck.iter().map(|c| format!("\"{}\"", c)).collect();
    let salts_items: Vec<String> = salts.iter().map(|s| format!("\"{}\"", s)).collect();

    // Compute real Merkle root
    let commitment_frs: Vec<Fr> = deck
        .iter()
        .zip(salts.iter())
        .map(|(card, salt)| crypto::commit_card(*card, salt))
        .collect();

    let mut leaves = [Fr::from(0u64); 64];
    for (i, c) in commitment_frs.iter().enumerate() {
        leaves[i] = *c;
    }
    let deck_root = crypto::compute_merkle_root(&leaves);

    // Pad revealed arrays
    let mut padded_revealed = vec![0u32; max_reveal];
    let mut padded_reveal_indices = vec![0u32; max_reveal];
    for (i, &idx) in reveal_indices.iter().enumerate().take(max_reveal) {
        padded_reveal_indices[i] = idx;
        padded_revealed[i] = deck[idx as usize];
    }

    let mut padded_used = vec![0u32; max_used];
    for (i, &idx) in previously_used.iter().enumerate().take(max_used) {
        padded_used[i] = idx;
    }

    let mut lines = Vec::new();
    lines.push(format!("deck = [{}]", deck_items.join(", ")));
    lines.push(format!("salts = [{}]", salts_items.join(", ")));
    lines.push(format!("deck_root = \"{}\"", crypto::fr_to_decimal_string(&deck_root)));
    lines.push(format!("num_revealed = \"{}\"", reveal_indices.len()));
    lines.push(format_field_array("revealed_cards", &padded_revealed));
    lines.push(format_field_array("revealed_indices", &padded_reveal_indices));
    lines.push(format!("num_previously_used = \"{}\"", previously_used.len()));
    lines.push(format_field_array("previously_used_indices", &padded_used));

    lines.join("\n")
}

/// Build Prover.toml for showdown_valid circuit.
fn build_showdown_prover_toml(
    hole_cards: &[(u32, u32)],
    board_cards: &[u32],
    salts: &[(String, String)],
    hand_commitments: &[String],
    winner_index: u32,
) -> String {
    let max_players = 6;
    let num_active = hole_cards.len();

    // Pad arrays to MAX_PLAYERS
    let mut card1 = vec![0u32; max_players];
    let mut card2 = vec![0u32; max_players];
    let mut salts1 = vec!["0".to_string(); max_players];
    let mut salts2 = vec!["0".to_string(); max_players];
    let mut commits = vec!["0".to_string(); max_players];

    for (i, ((c1, c2), (s1, s2))) in hole_cards.iter().zip(salts.iter()).enumerate() {
        card1[i] = *c1;
        card2[i] = *c2;
        salts1[i] = s1.clone();
        salts2[i] = s2.clone();
    }
    for (i, c) in hand_commitments.iter().enumerate().take(max_players) {
        commits[i] = c.clone();
    }

    let mut lines = Vec::new();
    lines.push(format!("num_active_players = \"{}\"", num_active));
    lines.push(format_field_array_str("hand_commitments", &commits));
    lines.push(format_field_array("board_cards", board_cards));
    lines.push(format_field_array("hole_card1", &card1));
    lines.push(format_field_array("hole_card2", &card2));
    lines.push(format_field_array_str("salts1", &salts1));
    lines.push(format_field_array_str("salts2", &salts2));
    lines.push(format!("winner_index = \"{}\"", winner_index));

    lines.join("\n")
}
