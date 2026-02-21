//! MPC integration for coordinator-to-node orchestration.
//!
//! Privacy model:
//! - Coordinator never generates or stores plaintext deck/salts.
//! - Every MPC node prepares and dispatches only its own private contribution.
//! - Nodes merge all source-party share fragments locally before proving.

use base64::Engine;
use serde::{Deserialize, Serialize};

/// Result from MPC proof generation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MpcProofResult {
    pub proof: Vec<u8>,
    pub public_inputs: Vec<String>,
    pub session_id: String,
}

#[derive(Clone, Debug)]
pub struct PreparedShareSets {
    pub share_set_ids: Vec<String>,
}

#[derive(Deserialize)]
struct NodeStatusResponse {
    #[allow(dead_code)]
    session_id: String,
    status: String,
}

#[derive(Deserialize)]
struct NodeProofResponse {
    #[allow(dead_code)]
    session_id: String,
    proof: String, // base64
    #[serde(default)]
    public_inputs: Vec<String>,
}

#[derive(Deserialize)]
struct NodePreparedSharesResponse {
    share_set_id: String,
}

/// Ask all nodes to prepare deal share sets.
pub async fn prepare_deal_from_nodes(
    node_endpoints: &[String],
    circuit_dir: &str,
    table_id: u32,
    players: &[String],
) -> Result<PreparedShareSets, String> {
    let client = reqwest::Client::new();
    let mut handles = Vec::with_capacity(node_endpoints.len());

    for (idx, endpoint) in node_endpoints.iter().enumerate() {
        let url = format!("{}/table/{}/prepare-deal", endpoint, table_id);
        let circuit_dir = circuit_dir.to_string();
        let players = players.to_vec();
        let client = client.clone();
        let handle = tokio::spawn(async move {
            let resp = client
                .post(&url)
                .json(&serde_json::json!({
                    "players": players,
                    "circuit_dir": circuit_dir,
                }))
                .send()
                .await
                .map_err(|e| format!("failed to call node {} prepare-deal: {}", idx, e))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "unable to read response body".to_string());
                return Err(format!(
                    "node {} prepare-deal rejected request: HTTP {}: {}",
                    idx, status, body
                ));
            }

            let prepared: NodePreparedSharesResponse = resp.json().await.map_err(|e| {
                format!("failed to parse node {} prepare-deal response: {}", idx, e)
            })?;

            Ok::<(usize, String), String>((idx, prepared.share_set_id))
        });
        handles.push(handle);
    }

    collect_prepared_share_sets(handles, node_endpoints.len()).await
}

/// Ask all nodes to prepare reveal share sets.
pub async fn prepare_reveal_from_nodes(
    node_endpoints: &[String],
    circuit_dir: &str,
    table_id: u32,
    phase: &str,
    previously_used_indices: &[u32],
    deck_root: &str,
) -> Result<PreparedShareSets, String> {
    let client = reqwest::Client::new();
    let mut handles = Vec::with_capacity(node_endpoints.len());

    for (idx, endpoint) in node_endpoints.iter().enumerate() {
        let url = format!("{}/table/{}/prepare-reveal/{}", endpoint, table_id, phase);
        let circuit_dir = circuit_dir.to_string();
        let deck_root = deck_root.to_string();
        let previously_used_indices = previously_used_indices.to_vec();
        let client = client.clone();
        let handle = tokio::spawn(async move {
            let resp = client
                .post(&url)
                .json(&serde_json::json!({
                    "circuit_dir": circuit_dir,
                    "previously_used_indices": previously_used_indices,
                    "deck_root": deck_root,
                }))
                .send()
                .await
                .map_err(|e| format!("failed to call node {} prepare-reveal: {}", idx, e))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "unable to read response body".to_string());
                return Err(format!(
                    "node {} prepare-reveal rejected request: HTTP {}: {}",
                    idx, status, body
                ));
            }

            let prepared: NodePreparedSharesResponse = resp.json().await.map_err(|e| {
                format!(
                    "failed to parse node {} prepare-reveal response: {}",
                    idx, e
                )
            })?;

            Ok::<(usize, String), String>((idx, prepared.share_set_id))
        });
        handles.push(handle);
    }

    collect_prepared_share_sets(handles, node_endpoints.len()).await
}

/// Ask all nodes to prepare showdown share sets.
pub async fn prepare_showdown_from_nodes(
    node_endpoints: &[String],
    circuit_dir: &str,
    table_id: u32,
    board_indices: &[u32],
    num_active_players: u32,
    hand_commitments: &[String],
    deck_root: &str,
) -> Result<PreparedShareSets, String> {
    let client = reqwest::Client::new();
    let mut handles = Vec::with_capacity(node_endpoints.len());

    for (idx, endpoint) in node_endpoints.iter().enumerate() {
        let url = format!("{}/table/{}/prepare-showdown", endpoint, table_id);
        let circuit_dir = circuit_dir.to_string();
        let board_indices = board_indices.to_vec();
        let hand_commitments = hand_commitments.to_vec();
        let deck_root = deck_root.to_string();
        let client = client.clone();
        let handle = tokio::spawn(async move {
            let resp = client
                .post(&url)
                .json(&serde_json::json!({
                    "circuit_dir": circuit_dir,
                    "board_indices": board_indices,
                    "num_active_players": num_active_players,
                    "hand_commitments": hand_commitments,
                    "deck_root": deck_root,
                }))
                .send()
                .await
                .map_err(|e| format!("failed to call node {} prepare-showdown: {}", idx, e))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "unable to read response body".to_string());
                return Err(format!(
                    "node {} prepare-showdown rejected request: HTTP {}: {}",
                    idx, status, body
                ));
            }

            let prepared: NodePreparedSharesResponse = resp.json().await.map_err(|e| {
                format!(
                    "failed to parse node {} prepare-showdown response: {}",
                    idx, e
                )
            })?;

            Ok::<(usize, String), String>((idx, prepared.share_set_id))
        });
        handles.push(handle);
    }

    collect_prepared_share_sets(handles, node_endpoints.len()).await
}

/// Dispatch all prepared share sets and trigger MPC proof generation.
pub async fn generate_proof_from_share_sets(
    table_id: u32,
    share_set_ids: &[String],
    session_id: &str,
    circuit_name: &str,
    circuit_dir: &str,
    node_endpoints: &[String],
) -> Result<MpcProofResult, String> {
    dispatch_share_sets_from_nodes(
        node_endpoints,
        table_id,
        share_set_ids,
        session_id,
        circuit_name,
    )
    .await?;
    trigger_and_collect_proof(session_id, circuit_name, circuit_dir, node_endpoints).await
}

#[derive(Deserialize)]
struct NodePermLookupResponse {
    mapped_indices: Vec<u32>,
    salts: Vec<String>,
}

/// Resolve hole cards for a player by chaining permutation lookups across nodes
/// and summing salts from all nodes at the original dealt positions.
///
/// Returns (card_values, combined_salts) for the given deck positions.
pub async fn resolve_hole_cards(
    node_endpoints: &[String],
    table_id: u32,
    card_positions: &[u32],
) -> Result<(Vec<u32>, Vec<String>), String> {
    if node_endpoints.len() != 3 {
        return Err(format!(
            "expected 3 MPC nodes, got {}",
            node_endpoints.len()
        ));
    }

    let client = reqwest::Client::new();

    // Step 1: Query all 3 nodes in parallel with original positions to get salts.
    // Also use node2's mapped_indices as the first step of the permutation chain.
    let mut salt_handles = Vec::with_capacity(3);
    for (i, endpoint) in node_endpoints.iter().enumerate() {
        let url = format!("{}/table/{}/perm-lookup", endpoint, table_id);
        let client = client.clone();
        let positions = card_positions.to_vec();
        let handle = tokio::spawn(async move {
            let resp = client
                .post(&url)
                .json(&serde_json::json!({ "indices": positions }))
                .send()
                .await
                .map_err(|e| format!("node {} perm-lookup failed: {}", i, e))?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "unable to read body".to_string());
                return Err(format!(
                    "node {} perm-lookup rejected: HTTP {}: {}",
                    i, status, body
                ));
            }
            let data: NodePermLookupResponse = resp
                .json()
                .await
                .map_err(|e| format!("node {} perm-lookup parse failed: {}", i, e))?;
            Ok::<(usize, NodePermLookupResponse), String>((i, data))
        });
        salt_handles.push(handle);
    }

    let mut node_responses: Vec<Option<NodePermLookupResponse>> = vec![None, None, None];
    for handle in salt_handles {
        let (idx, resp) = handle
            .await
            .map_err(|e| format!("perm-lookup join error: {}", e))??;
        node_responses[idx] = Some(resp);
    }

    let resp0 = node_responses[0].take().ok_or("missing node 0 response")?;
    let resp1 = node_responses[1].take().ok_or("missing node 1 response")?;
    let resp2 = node_responses[2].take().ok_or("missing node 2 response")?;

    // Sum salts from all 3 nodes (all at the same original positions).
    // Salts are u64 values; sum fits in u128, well below BN254 modulus.
    let num_cards = card_positions.len();
    let mut combined_salts = Vec::with_capacity(num_cards);
    for i in 0..num_cards {
        let s0: u128 = resp0.salts[i]
            .parse::<u64>()
            .map_err(|e| format!("node0 salt parse: {}", e))?
            .into();
        let s1: u128 = resp1.salts[i]
            .parse::<u64>()
            .map_err(|e| format!("node1 salt parse: {}", e))?
            .into();
        let s2: u128 = resp2.salts[i]
            .parse::<u64>()
            .map_err(|e| format!("node2 salt parse: {}", e))?
            .into();
        combined_salts.push(format!("{}", s0 + s1 + s2));
    }

    // Step 2: Chain permutation lookups: node2 → node1 → node0.
    // We already have node2's mapped_indices from step 1.
    let step1 = resp2.mapped_indices;

    // Query node1 with node2's mapped indices.
    let step2 = query_perm_lookup(&client, &node_endpoints[1], table_id, &step1)
        .await?
        .mapped_indices;

    // Query node0 with node1's result → final card values.
    let final_cards = query_perm_lookup(&client, &node_endpoints[0], table_id, &step2)
        .await?
        .mapped_indices;

    Ok((final_cards, combined_salts))
}

async fn query_perm_lookup(
    client: &reqwest::Client,
    endpoint: &str,
    table_id: u32,
    indices: &[u32],
) -> Result<NodePermLookupResponse, String> {
    let url = format!("{}/table/{}/perm-lookup", endpoint, table_id);
    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "indices": indices }))
        .send()
        .await
        .map_err(|e| format!("perm-lookup to {} failed: {}", url, e))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp
            .text()
            .await
            .unwrap_or_else(|_| "unable to read body".to_string());
        return Err(format!(
            "perm-lookup to {} rejected: HTTP {}: {}",
            url, status, body
        ));
    }
    resp.json()
        .await
        .map_err(|e| format!("perm-lookup parse from {} failed: {}", url, e))
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

async fn collect_prepared_share_sets(
    handles: Vec<tokio::task::JoinHandle<Result<(usize, String), String>>>,
    expected_len: usize,
) -> Result<PreparedShareSets, String> {
    let mut ordered = vec![String::new(); expected_len];
    for handle in handles {
        let (idx, share_set_id) = handle
            .await
            .map_err(|e| format!("prepare task join error: {}", e))??;
        if idx >= ordered.len() {
            return Err(format!("prepare task returned out-of-range index {}", idx));
        }
        ordered[idx] = share_set_id;
    }

    if ordered.iter().any(|id| id.is_empty()) {
        return Err("missing share_set_id for one or more nodes".to_string());
    }

    Ok(PreparedShareSets {
        share_set_ids: ordered,
    })
}

async fn dispatch_share_sets_from_nodes(
    node_endpoints: &[String],
    table_id: u32,
    share_set_ids: &[String],
    session_id: &str,
    circuit_name: &str,
) -> Result<(), String> {
    if node_endpoints.len() != share_set_ids.len() {
        return Err(format!(
            "node count ({}) does not match share_set count ({})",
            node_endpoints.len(),
            share_set_ids.len()
        ));
    }

    let client = reqwest::Client::new();
    let mut handles = Vec::with_capacity(node_endpoints.len());

    for (idx, endpoint) in node_endpoints.iter().enumerate() {
        let url = format!("{}/table/{}/dispatch-shares", endpoint, table_id);
        let share_set_id = share_set_ids[idx].clone();
        let session_id = session_id.to_string();
        let circuit_name = circuit_name.to_string();
        let client = client.clone();
        let handle = tokio::spawn(async move {
            let resp = client
                .post(&url)
                .json(&serde_json::json!({
                    "share_set_id": share_set_id,
                    "proof_session_id": session_id,
                    "circuit_name": circuit_name,
                }))
                .send()
                .await
                .map_err(|e| format!("failed to call node {} dispatch-shares: {}", idx, e))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "unable to read response body".to_string());
                return Err(format!(
                    "node {} dispatch-shares rejected request: HTTP {}: {}",
                    idx, status, body
                ));
            }
            Ok::<(), String>(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle
            .await
            .map_err(|e| format!("dispatch join error: {}", e))??;
    }

    Ok(())
}

async fn trigger_and_collect_proof(
    session_id: &str,
    circuit_name: &str,
    circuit_dir: &str,
    node_endpoints: &[String],
) -> Result<MpcProofResult, String> {
    if node_endpoints.is_empty() {
        return Err("no MPC node endpoints configured".to_string());
    }

    let client = reqwest::Client::new();

    // Node expects CRS directory (it appends bn254_g1.dat internally).
    let crs_dir = std::env::var("CRS_DIR").unwrap_or_else(|_| "./crs".to_string());

    let mut handles = Vec::new();
    for (i, endpoint) in node_endpoints.iter().enumerate() {
        let url = format!("{}/session/{}/generate", endpoint, session_id);
        let client = client.clone();
        let circuit_dir = circuit_dir.to_string();
        let crs_dir = crs_dir.clone();
        let handle = tokio::spawn(async move {
            let resp = client
                .post(&url)
                .json(&serde_json::json!({
                    "circuit_dir": circuit_dir,
                    "crs_path": crs_dir,
                }))
                .send()
                .await
                .map_err(|e| format!("failed to trigger node {}: {}", i, e))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "unable to read response body".to_string());
                return Err(format!(
                    "node {} trigger failed: HTTP {}: {}",
                    i, status, body
                ));
            }
            Ok::<(), String>(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await.map_err(|e| format!("join error: {}", e))??;
    }

    // Poll node 0 for proof completion.
    let proof_node = &node_endpoints[0];
    let max_polls = if circuit_name == "showdown_valid" {
        900
    } else {
        300
    };
    for _ in 0..max_polls {
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
                let proof_url = format!("{}/session/{}/proof", proof_node, session_id);
                let proof_resp = client
                    .get(&proof_url)
                    .send()
                    .await
                    .map_err(|e| format!("failed to fetch proof: {}", e))?;

                if !proof_resp.status().is_success() {
                    let status = proof_resp.status();
                    let body = proof_resp
                        .text()
                        .await
                        .unwrap_or_else(|_| "unable to read response body".to_string());
                    return Err(format!("proof fetch failed: HTTP {}: {}", status, body));
                }

                let proof_data: NodeProofResponse = proof_resp
                    .json()
                    .await
                    .map_err(|e| format!("failed to parse proof: {}", e))?;

                let proof_bytes = base64::engine::general_purpose::STANDARD
                    .decode(&proof_data.proof)
                    .map_err(|e| format!("failed to decode proof: {}", e))?;

                return Ok(MpcProofResult {
                    proof: proof_bytes,
                    public_inputs: proof_data.public_inputs,
                    session_id: session_id.to_string(),
                });
            }
            s if s.starts_with("failed") => {
                return Err(format!("proof generation failed: {}", s));
            }
            _ => {}
        }
    }

    Err(format!(
        "[{}] proof generation timed out after {} seconds",
        session_id, max_polls
    ))
}
